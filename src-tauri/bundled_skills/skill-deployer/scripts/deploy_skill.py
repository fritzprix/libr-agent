#!/usr/bin/env python3
"""
Deploy a validated LibrAgent skill to user_skills/, workspace .libragent/skills/, or assistant scope.

Usage:
    python deploy_skill.py <skill-source-dir> --scope global
    python deploy_skill.py <skill-source-dir> --scope workspace --workspace <session-workspace-root>
    python deploy_skill.py <skill-source-dir> --scope assistant --assistant-id <id>

Deployment always runs strict validation (frontmatter + path + layout) before and after copy.
On post-deploy validation failure, the target directory is removed (rollback).

Never deploys to system_skills/ (managed bundled mirror).
"""

from __future__ import annotations

import argparse
import platform
import shutil
import sys
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from validate_skill import ValidationReport


def bundled_skills_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def load_validator():
    creator_scripts = bundled_skills_root() / "skill-creator" / "scripts"
    validator_path = creator_scripts / "validate_skill.py"
    if not validator_path.is_file():
        print(
            "[DEPLOY-SETUP-001] ERROR: validate_skill.py not found.\n"
            f"  Path: {validator_path}\n"
            "  Fix: Deploy skill-creator alongside skill-deployer under bundled_skills/.",
            file=sys.stderr,
        )
        sys.exit(2)

    sys.path.insert(0, str(creator_scripts))
    from validate_skill import format_detailed_report, get_skill_name, validate_skill_path

    return validate_skill_path, format_detailed_report, get_skill_name


def default_data_dir() -> Path:
    system = platform.system()
    home = Path.home()

    if system == "Windows":
        import os

        appdata = Path(os.environ.get("APPDATA", str(home / "AppData" / "Roaming")))
        return appdata / "com.fritzprix.libragent"

    if system == "Darwin":
        return home / "Library" / "Application Support" / "com.fritzprix.libragent"

    return home / ".local" / "share" / "com.fritzprix.libragent"


def abort_validation(
    report: ValidationReport,
    phase: str,
    format_detailed_report,
    exit_code: int = 1,
) -> None:
    print(format_detailed_report(report, phase), file=sys.stderr)
    print(f"\nDeploy aborted at phase: {phase}", file=sys.stderr)
    print(
        "Next step: python <skill-creator>/scripts/validate_skill.py "
        "<skill-folder> --strict --detailed",
        file=sys.stderr,
    )
    raise SystemExit(exit_code)


def run_strict_validation(
    skill_path: Path,
    phase: str,
    validate_skill_path,
    format_detailed_report,
) -> ValidationReport:
    report = validate_skill_path(skill_path, strict=True)
    if not report.fail_on_warnings(True):
        abort_validation(report, phase, format_detailed_report)
    return report


def normalize_workspace_skills_parent(workspace: Path) -> Path:
    resolved = workspace.resolve()
    if resolved.name == "skills" and resolved.parent.name == ".libragent":
        return resolved
    return resolved / ".libragent" / "skills"


def resolve_target_dir(
    scope: str,
    skill_name: str,
    data_dir: Path,
    workspace: Path | None,
    assistant_id: str | None,
) -> Path:
    if scope == "global":
        return data_dir / "user_skills" / skill_name

    if scope == "workspace":
        if workspace is None:
            print(
                "[DEPLOY-ARGS-001] ERROR: --workspace is required for --scope workspace.\n"
                "  Fix: Pass the session workspace root from the system prompt ## Workspace section.",
                file=sys.stderr,
            )
            sys.exit(2)
        return normalize_workspace_skills_parent(workspace) / skill_name

    if scope == "assistant":
        if not assistant_id:
            print(
                "[DEPLOY-ARGS-002] ERROR: --assistant-id is required for --scope assistant.\n"
                "  Fix: Pass the assistant ID from the Assistants settings panel.",
                file=sys.stderr,
            )
            sys.exit(2)
        return data_dir / "assistants" / assistant_id / "skills" / skill_name

    print(f"[DEPLOY-ARGS-003] ERROR: Unknown scope '{scope}'.", file=sys.stderr)
    sys.exit(2)


def assert_target_allowed(target_dir: Path) -> None:
    parts = [part.lower() for part in target_dir.resolve().parts]
    if "system_skills" in parts:
        print(
            "[DEPLOY-TARGET-001] ERROR: Refusing to deploy to system_skills/.\n"
            "  Fix: Use --scope global (user_skills/) for custom skills. "
            "App-bundled skills belong in src-tauri/bundled_skills/ (repo workflow).",
            file=sys.stderr,
        )
        sys.exit(1)

    resolved_str = str(target_dir.resolve()).lower()
    if ("\\skills\\" in resolved_str or "/skills/" in resolved_str) and "user_skills" not in parts:
        if "assistants" not in parts and (
            "com.fritzprix.libragent" in parts or "libragent" in resolved_str
        ):
            print(
                "[DEPLOY-TARGET-002] ERROR: Refusing legacy global skills/ path.\n"
                "  Fix: Target must be .../user_skills/<skill-name>/.\n"
                "  Use: deploy_skill.py <skill> --scope global",
                file=sys.stderr,
            )
            sys.exit(1)


def assert_data_dir_exists(data_dir: Path) -> None:
    if data_dir.exists():
        return
    print(
        "[DEPLOY-TARGET-003] ERROR: LibrAgent data directory does not exist.\n"
        f"  Path: {data_dir}\n"
        "  Fix: Start LibrAgent once to create app data, or pass --data-dir with the correct path.",
        file=sys.stderr,
    )
    sys.exit(1)


def copy_skill_tree(source: Path, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(source, target)


def rollback_target(target: Path, reason: str) -> None:
    if not target.exists():
        return
    shutil.rmtree(target)
    print(
        f"[DEPLOY-ROLLBACK] Removed incomplete deploy at:\n  {target}\n  Reason: {reason}",
        file=sys.stderr,
    )


def main() -> int:
    validate_skill_path, format_detailed_report, get_skill_name = load_validator()

    parser = argparse.ArgumentParser(
        description="Deploy a validated LibrAgent skill (strict validation before and after copy)"
    )
    parser.add_argument("skill_source", help="Path to the skill folder containing SKILL.md")
    parser.add_argument(
        "--scope",
        required=True,
        choices=("global", "workspace", "assistant"),
        help="Deployment scope",
    )
    parser.add_argument(
        "--workspace",
        help="Workspace root (session directory) for --scope workspace",
    )
    parser.add_argument(
        "--assistant-id",
        help="Assistant ID for --scope assistant",
    )
    parser.add_argument(
        "--data-dir",
        help="LibrAgent data directory (default: OS-specific com.fritzprix.libragent)",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Replace existing target directory",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and print target path without copying",
    )
    args = parser.parse_args()

    source = Path(args.skill_source).resolve()
    if not source.is_dir():
        print(
            "[DEPLOY-SOURCE-001] ERROR: Skill source is not a directory.\n"
            f"  Path: {source}\n"
            "  Fix: Pass the folder that contains SKILL.md.",
            file=sys.stderr,
        )
        return 2

    if not (source / "SKILL.md").is_file():
        print(
            "[DEPLOY-SOURCE-002] ERROR: SKILL.md not found in source directory.\n"
            f"  Path: {source}\n"
            "  Fix: Create SKILL.md or point deploy_skill.py at the correct skill folder.",
            file=sys.stderr,
        )
        return 2

    source_report = run_strict_validation(
        source,
        "Pre-deploy validation (source)",
        validate_skill_path,
        format_detailed_report,
    )
    skill_name = get_skill_name(source_report)
    if not skill_name:
        print(
            "[DEPLOY-SOURCE-003] ERROR: Could not read skill name from validated frontmatter.\n"
            "  Fix: Ensure frontmatter contains name: <folder-name>.",
            file=sys.stderr,
        )
        return 1

    data_dir = Path(args.data_dir).resolve() if args.data_dir else default_data_dir()
    workspace = Path(args.workspace).resolve() if args.workspace else None

    assert_data_dir_exists(data_dir)

    target = resolve_target_dir(
        args.scope,
        skill_name,
        data_dir,
        workspace,
        args.assistant_id,
    )
    assert_target_allowed(target)

    if target.exists() and not args.overwrite and not args.dry_run:
        print(
            "[DEPLOY-TARGET-004] ERROR: Target already exists.\n"
            f"  Path: {target}\n"
            "  Fix: Re-run with --overwrite to replace the existing skill.",
            file=sys.stderr,
        )
        return 1

    print("=== Deploy plan ===")
    print(f"Scope: {args.scope}")
    print(f"Skill: {skill_name}")
    print(f"Source: {source}")
    print(f"Target: {target}")
    print("Validation: strict (pre-deploy passed)")

    if args.dry_run:
        print("Dry run: no files copied.")
        return 0

    try:
        copy_skill_tree(source, target)
    except OSError as exc:
        rollback_target(target, f"copy failed: {exc}")
        print(
            "[DEPLOY-COPY-001] ERROR: Failed to copy skill files.\n"
            f"  Details: {exc}\n"
            "  Fix: Check disk permissions and that the target path is writable.",
            file=sys.stderr,
        )
        return 1

    post_report = validate_skill_path(target, strict=True)
    if not post_report.fail_on_warnings(True):
        rollback_target(target, "post-deploy strict validation failed")
        print(format_detailed_report(post_report, "Post-deploy validation (deployed copy)"), file=sys.stderr)
        print(
            "\nDeploy failed: rolled back target directory.\n"
            "Next step: fix the source skill, re-run validate_skill.py --strict --detailed, then deploy again.",
            file=sys.stderr,
        )
        return 1

    print(format_detailed_report(post_report, "Post-deploy validation (deployed copy)"))
    print("\nDeploy complete. Skill becomes active on the next agent turn.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
