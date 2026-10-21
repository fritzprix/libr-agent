#!/usr/bin/env python3
"""
Deploy, inspect, update, or remove a validated LibrAgent skill (local directory or GitHub URL)
for user_skills/ (global), workspace .libragent/skills/, or assistant scope.

Usage:
    # Local folder deploy
    python deploy_skill.py <skill-source-dir> --scope global
    python deploy_skill.py <skill-source-dir> --scope workspace --workspace <session-workspace-root>
    python deploy_skill.py <skill-source-dir> --scope assistant --assistant-id <id>

    # GitHub URL inspect / deploy (repo URL, /tree/<ref>/<subpath>, or ?ref=<ref>&path=<subpath>)
    python deploy_skill.py <github-url> --inspect
    python deploy_skill.py <github-url> --scope workspace --workspace <root> [--skill <name>] [--approve-scripts]

    # Update or remove an installed skill
    python deploy_skill.py --update <skill-name> --scope workspace --workspace <root> [--ref <branch>] [--approve-scripts]
    python deploy_skill.py --remove <skill-name> --scope workspace --workspace <root>

Deployment always runs strict validation (frontmatter + path + layout) before and after copy.
On post-deploy validation failure, the target directory is rolled back (or restored from backup).
Never deploys to or removes from system_skills/ (managed bundled mirror).
"""

from __future__ import annotations

import argparse
import contextlib
import platform
import re
import shutil
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from github_fetch import (
    IGNORED_WALK_DIRS,
    PROVENANCE_FILE_NAME,
    STRIPPED_SKILL_ROOT_FILES_LOWER,
    GitHubDeployError,
    ResolvedGitHubSpec,
    build_github_provenance,
    build_local_provenance,
    discover_skill_dirs,
    fetch_github_repo_at_commit,
    is_commit_sha,
    is_github_source,
    parse_github_url,
    read_provenance,
    relative_skill_subpath,
    resolve_github_spec,
    scan_skill_scripts,
    write_provenance,
)

if TYPE_CHECKING:
    from validate_skill import ValidationReport

ValidateSkillFn = Callable[[Path, bool], "ValidationReport"]
FormatDetailedReportFn = Callable[["ValidationReport", str], str]
GetSkillNameFn = Callable[["ValidationReport"], str | None]
ProvenanceBuilderFn = Callable[[dict[str, Any] | None], dict[str, Any]]

SAFE_SKILL_NAME_RE = re.compile(r"^[a-z0-9-]+$")


def bundled_skills_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def load_validator() -> tuple[ValidateSkillFn, FormatDetailedReportFn, GetSkillNameFn]:
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

    if str(creator_scripts) not in sys.path:
        sys.path.insert(0, str(creator_scripts))
    with contextlib.suppress(ImportError, OSError):
        from io_utils import configure_stdio

        configure_stdio()

    from validate_skill import (
        format_detailed_report,
        get_skill_name,
        validate_skill_path,
    )

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
    format_detailed_report: FormatDetailedReportFn,
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
    validate_skill_path: ValidateSkillFn,
    format_detailed_report: FormatDetailedReportFn,
    allow_stripped_root_files: bool = False,
) -> ValidationReport:
    report = validate_skill_path(skill_path, strict=True)
    if allow_stripped_root_files and report.issues:
        stripped_issues = [
            issue for issue in report.issues if issue.code == "SKILL-LAYOUT-FORBIDDEN-FILE"
        ]
        if stripped_issues:
            report.issues = [
                issue for issue in report.issues if issue.code != "SKILL-LAYOUT-FORBIDDEN-FILE"
            ]
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
            "[DEPLOY-TARGET-001] ERROR: Refusing to modify system_skills/.\n"
            "  Fix: Use --scope global (user_skills/) for custom skills. "
            "App-bundled skills belong in src-tauri/bundled_skills/ (repo workflow).",
            file=sys.stderr,
        )
        sys.exit(1)

    if (
        len(parts) >= 3
        and parts[-2] == "skills"
        and parts[-3] in {"com.fritzprix.libragent", "libragent"}
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


def validate_skill_identifier(skill_name: str) -> str:
    cleaned = skill_name.strip()
    if (
        not cleaned
        or "/" in cleaned
        or "\\" in cleaned
        or cleaned in {".", ".."}
        or not SAFE_SKILL_NAME_RE.match(cleaned)
    ):
        print(
            f"[DEPLOY-ARGS-004] ERROR: Invalid skill name '{skill_name}'.\n"
            "  Fix: Skill name must be hyphen-case (lowercase letters, digits, hyphens only).",
            file=sys.stderr,
        )
        sys.exit(2)
    return cleaned


def copy_skill_tree(source: Path, target: Path) -> None:
    resolved_source = source.resolve()
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        shutil.rmtree(target)

    def _ignore(cur_dir: str, names: list[str]) -> set[str]:
        ignored: set[str] = set()
        is_root = Path(cur_dir).resolve() == resolved_source
        for name in names:
            lower = name.lower()
            if (
                name in IGNORED_WALK_DIRS
                or lower in IGNORED_WALK_DIRS
                or lower.endswith((".pyc", ".pyo"))
                or (is_root and lower in STRIPPED_SKILL_ROOT_FILES_LOWER)
            ):
                ignored.add(name)
        return ignored

    shutil.copytree(resolved_source, target, ignore=_ignore)


def rollback_target(target: Path, backup_dir: Path | None, reason: str) -> None:
    if target.exists():
        shutil.rmtree(target)
    if backup_dir is not None and backup_dir.exists():
        shutil.copytree(backup_dir, target)
        print(
            f"[DEPLOY-ROLLBACK] Restored previous skill at:\n  {target}\n  Reason: {reason}",
            file=sys.stderr,
        )
        return
    print(
        f"[DEPLOY-ROLLBACK] Removed incomplete deploy at:\n  {target}\n  Reason: {reason}",
        file=sys.stderr,
    )


def print_review_summary(
    skill_name: str,
    description: str,
    source_label: str,
    scripts: list[str],
    commit: str | None = None,
    ref: str | None = None,
    subpath: str | None = None,
) -> None:
    print("=== Skill Review ===")
    print(f"Skill: {skill_name}")
    print(f"Description: {description}")
    print(f"Source: {source_label}")
    if commit:
        ref_display = ref or "HEAD"
        subpath_display = subpath or "."
        print(f"Pinned commit: {commit} (ref: {ref_display}, subpath: {subpath_display})")
        print(
            "Trust note: Skills are loaded into the system prompt — review SKILL.md instructions "
            "from third-party repositories before use."
        )
    if scripts:
        print(f"Scripts / Executables ({len(scripts)}):")
        for rel in scripts:
            print(f"  - {rel}")
    else:
        print("Scripts / Executables: None (prompt/reference-only skill)")


def enforce_script_approval(
    skill_name: str,
    scripts: list[str],
    require_approval: bool,
    approve_scripts: bool,
    dry_run: bool,
) -> None:
    if not require_approval or not scripts or approve_scripts or dry_run:
        return
    script_lines = "\n".join(f"    - {rel}" for rel in scripts)
    print(
        f"\n[DEPLOY-APPROVAL-001] APPROVAL REQUIRED: Skill '{skill_name}' contains scripts or executables:\n"
        f"{script_lines}\n"
        "  Fix: Show the user the skill description and the scripts/executables listed above, "
        "get explicit user confirmation, then re-run with --approve-scripts.",
        file=sys.stderr,
    )
    raise SystemExit(4)


def select_github_skill(
    repo_root: Path,
    spec: ResolvedGitHubSpec,
    skill_filter: str | None,
    inspect_only: bool,
    validate_skill_path: ValidateSkillFn,
    get_skill_name: GetSkillNameFn,
) -> tuple[Path, str]:
    skill_dirs = discover_skill_dirs(repo_root, spec.subpath)
    if not skill_dirs:
        subpath_note = f" under subpath '{spec.subpath}'" if spec.subpath else ""
        raise GitHubDeployError(
            "DEPLOY-GITHUB-003",
            f"No SKILL.md found in {spec.repo_url}@{spec.commit[:12]}{subpath_note}.",
            "Point to a repository or --subpath that contains a valid SKILL.md folder.",
        )

    candidates: list[dict[str, Any]] = []
    for sdir in skill_dirs:
        report = validate_skill_path(sdir, strict=True)
        fm = report.frontmatter if isinstance(report.frontmatter, dict) else {}
        name = get_skill_name(report) or sdir.name
        desc = str(fm.get("description", "")).strip() if fm else ""
        rel_sub = relative_skill_subpath(repo_root, sdir)
        scripts = scan_skill_scripts(sdir)
        candidates.append(
            {
                "dir": sdir,
                "name": name,
                "folder": sdir.name,
                "subpath": rel_sub,
                "description": desc,
                "valid": report.fail_on_warnings(True),
                "issues": [issue.code for issue in report.issues],
                "scripts": scripts,
            }
        )

    if skill_filter:
        wanted = skill_filter.strip().lower()
        matched = [
            c
            for c in candidates
            if c["name"].lower() == wanted
            or c["folder"].lower() == wanted
            or c["subpath"].lower() == wanted
        ]
        if not matched:
            available = ", ".join(c["name"] for c in candidates)
            raise GitHubDeployError(
                "DEPLOY-SELECT-002",
                f"Skill '{skill_filter}' not found in {spec.repo_url}@{spec.commit[:12]}. Available: {available}",
                "Pass one of the available skill names via --skill <name>.",
                exit_code=2,
            )
        if len(matched) > 1:
            subpaths = ", ".join(c["subpath"] or "." for c in matched)
            raise GitHubDeployError(
                "DEPLOY-SELECT-003",
                f"Multiple skills matched '{skill_filter}' ({subpaths}).",
                "Disambiguate with --subpath <relative-path>.",
                exit_code=3,
            )
        chosen = matched[0]
        return chosen["dir"], chosen["subpath"]

    if len(candidates) > 1:
        print(f"=== Discovered Skills in {spec.repo_url} @ {spec.commit} (ref: {spec.ref}) ===")
        for idx, c in enumerate(candidates, start=1):
            status = "valid" if c["valid"] else f"invalid ({', '.join(c['issues'])})"
            scripts_info = (
                f"scripts: {', '.join(c['scripts'])}" if c["scripts"] else "prompt-only"
            )
            subpath_label = c["subpath"] or "."
            short_desc = (
                c["description"][:120] + "..."
                if len(c["description"]) > 120
                else c["description"]
            )
            print(f"{idx}. {c['name']} (subpath: {subpath_label}) [{status}] [{scripts_info}]")
            if short_desc:
                print(f"   Description: {short_desc}")

        if inspect_only:
            print(
                "\nMultiple skills available. Ask the user which skill to install, "
                "then run deploy_skill.py with --skill <name> (and --scope <scope>)."
            )
            raise SystemExit(0)

        print(
            f"\n[DEPLOY-SELECT-001] ERROR: Found {len(candidates)} skills in {spec.repo_url}.\n"
            "  Fix: Show the numbered list above to the user, let them pick one, "
            "then re-run with --skill <name> (or --subpath <path>).",
            file=sys.stderr,
        )
        raise SystemExit(3)

    chosen = candidates[0]
    return chosen["dir"], chosen["subpath"]


def execute_deploy_to_target(
    source_dir: Path,
    target: Path,
    scope: str,
    skill_name: str,
    description: str,
    source_label: str,
    scripts: list[str],
    provenance_builder: ProvenanceBuilderFn,
    overwrite: bool,
    dry_run: bool,
    require_approval: bool,
    approve_scripts: bool,
    validate_skill_path: ValidateSkillFn,
    format_detailed_report: FormatDetailedReportFn,
    commit: str | None = None,
    ref: str | None = None,
    subpath: str | None = None,
) -> int:
    print_review_summary(
        skill_name=skill_name,
        description=description,
        source_label=source_label,
        scripts=scripts,
        commit=commit,
        ref=ref,
        subpath=subpath,
    )
    print()

    if source_dir.resolve() == target.resolve():
        print("=== Deploy plan ===")
        print(f"Scope: {scope}")
        print(f"Skill: {skill_name}")
        print(f"Target: {target}")
        print("Notice: Source and target are the same path (already located at destination).")
        print("Validation: strict (pre-deploy passed)")
        if not dry_run:
            existing_prov = read_provenance(target)
            write_provenance(target, provenance_builder(existing_prov))
        print("\nDeploy complete. Skill is valid and active at target location.")
        return 0

    enforce_script_approval(
        skill_name=skill_name,
        scripts=scripts,
        require_approval=require_approval,
        approve_scripts=approve_scripts,
        dry_run=dry_run,
    )

    if target.exists() and not overwrite and not dry_run:
        print(
            "[DEPLOY-TARGET-004] ERROR: Target already exists.\n"
            f"  Path: {target}\n"
            "  Fix: Re-run with --overwrite to replace the existing skill, or --update to update from provenance.",
            file=sys.stderr,
        )
        return 1

    print("=== Deploy plan ===")
    print(f"Scope: {scope}")
    print(f"Skill: {skill_name}")
    print(f"Source: {source_label}")
    print(f"Target: {target}")
    print("Validation: strict (pre-deploy passed)")

    if dry_run:
        print("Dry run: no files copied.")
        return 0

    existing_prov = read_provenance(target) if target.exists() else None
    backup_root: tempfile.TemporaryDirectory[str] | None = None
    backup_dir: Path | None = None
    if target.exists():
        backup_root = tempfile.TemporaryDirectory(prefix="skill-backup-")
        backup_dir = Path(backup_root.name) / target.name
        shutil.copytree(target, backup_dir)

    try:
        try:
            copy_skill_tree(source_dir, target)
            prov_payload = provenance_builder(existing_prov)
            prov_path = write_provenance(target, prov_payload)
        except OSError as exc:
            rollback_target(target, backup_dir, f"copy failed: {exc}")
            print(
                "[DEPLOY-COPY-001] ERROR: Failed to copy skill files.\n"
                f"  Details: {exc}\n"
                "  Fix: Check disk permissions and that the target path is writable.",
                file=sys.stderr,
            )
            return 1

        post_report = validate_skill_path(target, strict=True)
        if not post_report.fail_on_warnings(True):
            rollback_target(target, backup_dir, "post-deploy strict validation failed")
            print(
                format_detailed_report(post_report, "Post-deploy validation (deployed copy)"),
                file=sys.stderr,
            )
            print(
                "\nDeploy failed: rolled back target directory.\n"
                "Next step: fix the source skill, re-run validate_skill.py --strict --detailed, then deploy again.",
                file=sys.stderr,
            )
            return 1

        print(format_detailed_report(post_report, "Post-deploy validation (deployed copy)"))
        print(f"Provenance recorded: {prov_path}")
        print("\nDeploy complete. Skill becomes active on the next agent turn.")
        return 0
    finally:
        if backup_root is not None:
            backup_root.cleanup()


def handle_remove(
    skill_name_raw: str,
    scope: str | None,
    data_dir: Path,
    workspace: Path | None,
    assistant_id: str | None,
    dry_run: bool,
) -> int:
    if not scope:
        print(
            "[DEPLOY-ARGS-005] ERROR: --scope is required with --remove.\n"
            "  Fix: Pass --scope workspace, --scope global, or --scope assistant.",
            file=sys.stderr,
        )
        return 2

    skill_name = validate_skill_identifier(skill_name_raw)
    if scope in {"global", "assistant"}:
        assert_data_dir_exists(data_dir)

    target = resolve_target_dir(scope, skill_name, data_dir, workspace, assistant_id)
    assert_target_allowed(target)

    if not target.exists():
        print(
            f"[DEPLOY-REMOVE-001] ERROR: Skill '{skill_name}' not found at target.\n"
            f"  Path: {target}\n"
            "  Fix: Check --scope and skill name.",
            file=sys.stderr,
        )
        return 1

    if not target.is_dir() or not (target / "SKILL.md").is_file():
        print(
            f"[DEPLOY-REMOVE-002] ERROR: Target is not a valid skill directory (missing SKILL.md).\n"
            f"  Path: {target}\n"
            "  Fix: Refusing to delete a directory that does not contain SKILL.md.",
            file=sys.stderr,
        )
        return 1

    prov = read_provenance(target)
    print("=== Remove Skill ===")
    print(f"Scope: {scope}")
    print(f"Skill: {skill_name}")
    print(f"Target: {target}")
    if prov:
        src = prov.get("sourceUrl") or prov.get("sourcePath") or "unknown"
        commit = prov.get("commit")
        print(f"Provenance: {src}" + (f" @ {commit}" if commit else ""))

    if dry_run:
        print("Dry run: skill directory was not removed.")
        return 0

    shutil.rmtree(target)
    print("\nRemove complete. Skill will no longer appear in <available_skills> on the next turn.")
    return 0


def handle_update(
    skill_name_raw: str,
    scope: str | None,
    data_dir: Path,
    workspace: Path | None,
    assistant_id: str | None,
    ref_override: str | None,
    approve_scripts: bool,
    dry_run: bool,
    validate_skill_path: ValidateSkillFn,
    format_detailed_report: FormatDetailedReportFn,
    get_skill_name: GetSkillNameFn,
) -> int:
    if not scope:
        print(
            "[DEPLOY-ARGS-005] ERROR: --scope is required with --update.\n"
            "  Fix: Pass --scope workspace, --scope global, or --scope assistant.",
            file=sys.stderr,
        )
        return 2

    skill_name = validate_skill_identifier(skill_name_raw)
    if scope in {"global", "assistant"}:
        assert_data_dir_exists(data_dir)

    target = resolve_target_dir(scope, skill_name, data_dir, workspace, assistant_id)
    assert_target_allowed(target)

    if not target.is_dir() or not (target / "SKILL.md").is_file():
        print(
            f"[DEPLOY-UPDATE-001] ERROR: Installed skill '{skill_name}' not found at:\n"
            f"  {target}\n"
            "  Fix: Verify --scope and skill name, or install the skill first.",
            file=sys.stderr,
        )
        return 1

    prov = read_provenance(target)
    if not prov:
        print(
            f"[DEPLOY-UPDATE-002] ERROR: No {PROVENANCE_FILE_NAME} found in '{skill_name}'.\n"
            f"  Path: {target}\n"
            "  Fix: Re-install the skill from its GitHub URL or local path with --overwrite to record provenance.",
            file=sys.stderr,
        )
        return 1

    source_type = prov.get("sourceType")
    if source_type == "local":
        source_path_str = prov.get("sourcePath")
        if not source_path_str or not Path(source_path_str).is_dir():
            print(
                f"[DEPLOY-UPDATE-003] ERROR: Recorded local source path no longer exists: {source_path_str}\n"
                "  Fix: Re-run deploy_skill.py with the new source directory and --overwrite.",
                file=sys.stderr,
            )
            return 1
        source_dir = Path(source_path_str).resolve()
        allow_stripped = source_dir != target.resolve()
        source_report = run_strict_validation(
            source_dir,
            "Pre-deploy validation (update source)",
            validate_skill_path,
            format_detailed_report,
            allow_stripped_root_files=allow_stripped,
        )
        fm = source_report.frontmatter if isinstance(source_report.frontmatter, dict) else {}
        desc = str(fm.get("description", "")).strip()
        scripts = scan_skill_scripts(source_dir)
        return execute_deploy_to_target(
            source_dir=source_dir,
            target=target,
            scope=scope,
            skill_name=skill_name,
            description=desc,
            source_label=str(source_dir),
            scripts=scripts,
            provenance_builder=lambda existing: build_local_provenance(
                skill_name, source_dir, scripts, existing
            ),
            overwrite=True,
            dry_run=dry_run,
            require_approval=False,
            approve_scripts=approve_scripts,
            validate_skill_path=validate_skill_path,
            format_detailed_report=format_detailed_report,
        )

    if source_type != "github":
        print(
            f"[DEPLOY-UPDATE-002] ERROR: Unsupported provenance sourceType '{source_type}'.\n"
            "  Fix: Re-install the skill with --overwrite.",
            file=sys.stderr,
        )
        return 1

    owner = str(prov.get("owner") or "").strip()
    repo = str(prov.get("repo") or "").strip()
    if owner and repo:
        source_url = f"https://github.com/{owner}/{repo}"
    else:
        source_url = str(prov.get("sourceUrl") or prov.get("requestedUrl") or "").strip()
    recorded_ref = str(prov.get("ref") or "").strip() or None
    recorded_subpath = str(prov.get("subpath") or "").strip() or None
    previous_commit = str(prov.get("commit") or "").strip()

    if is_commit_sha(recorded_ref) and not ref_override:
        print(
            f"Notice: Skill '{skill_name}' was installed at a pinned commit SHA ({recorded_ref}).\n"
            "  Pass --ref <branch> with --update if you want to switch to tracking a branch."
        )

    try:
        parsed = parse_github_url(
            source_url,
            ref_override=ref_override or recorded_ref,
            subpath_override=recorded_subpath,
        )
        spec = resolve_github_spec(parsed)
        print(
            f"=== Update Check ({skill_name}) ===\n"
            f"Previous commit: {previous_commit or 'unknown'}\n"
            f"Latest commit:   {spec.commit} (ref: {spec.ref})"
        )
        with tempfile.TemporaryDirectory(prefix="skill-fetch-") as tmp:
            repo_root = fetch_github_repo_at_commit(spec, Path(tmp))
            selected_dir, skill_subpath = select_github_skill(
                repo_root=repo_root,
                spec=spec,
                skill_filter=skill_name,
                inspect_only=False,
                validate_skill_path=validate_skill_path,
                get_skill_name=get_skill_name,
            )
            source_report = run_strict_validation(
                selected_dir,
                "Pre-deploy validation (GitHub update source)",
                validate_skill_path,
                format_detailed_report,
            )
            validated_name = get_skill_name(source_report)
            if validated_name != skill_name:
                print(
                    f"[DEPLOY-UPDATE-004] ERROR: Upstream skill name changed from '{skill_name}' to '{validated_name}'.\n"
                    "  Fix: Remove the old skill with --remove and install the renamed skill fresh.",
                    file=sys.stderr,
                )
                return 1
            fm = source_report.frontmatter if isinstance(source_report.frontmatter, dict) else {}
            desc = str(fm.get("description", "")).strip()
            scripts = scan_skill_scripts(selected_dir)

            return execute_deploy_to_target(
                source_dir=selected_dir,
                target=target,
                scope=scope,
                skill_name=skill_name,
                description=desc,
                source_label=spec.repo_url,
                scripts=scripts,
                provenance_builder=lambda existing: build_github_provenance(
                    skill_name=skill_name,
                    spec=spec,
                    skill_subpath=skill_subpath,
                    scripts=scripts,
                    existing=existing,
                ),
                overwrite=True,
                dry_run=dry_run,
                require_approval=True,
                approve_scripts=approve_scripts,
                validate_skill_path=validate_skill_path,
                format_detailed_report=format_detailed_report,
                commit=spec.commit,
                ref=spec.ref,
                subpath=skill_subpath,
            )
    except GitHubDeployError as exc:
        print(exc.format(), file=sys.stderr)
        return exc.exit_code


def main() -> int:
    validate_skill_path, format_detailed_report, get_skill_name = load_validator()

    parser = argparse.ArgumentParser(
        description="Deploy, inspect, update, or remove a validated LibrAgent skill (local path or GitHub URL)"
    )
    parser.add_argument(
        "skill_source",
        nargs="?",
        help="Path to the local skill folder containing SKILL.md, or a GitHub URL (https://github.com/...)",
    )
    parser.add_argument(
        "--scope",
        choices=("global", "workspace", "assistant"),
        help="Deployment scope (required for install, --update, and --remove; optional for --inspect)",
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
        "--ref",
        help="Git branch, tag, or commit SHA when fetching from a GitHub URL",
    )
    parser.add_argument(
        "--subpath",
        help="Relative subdirectory inside the GitHub repository",
    )
    parser.add_argument(
        "--skill",
        help="Skill name to select when a GitHub repository contains multiple SKILL.md folders",
    )
    parser.add_argument(
        "--inspect",
        action="store_true",
        help="Fetch/validate and show discovered skills, description, and scripts without installing",
    )
    parser.add_argument(
        "--approve-scripts",
        action="store_true",
        help="Confirm user approval to install a skill that contains scripts or executables",
    )
    parser.add_argument(
        "--update",
        metavar="SKILL_NAME",
        help="Re-fetch and update an installed skill from its recorded provenance",
    )
    parser.add_argument(
        "--remove",
        metavar="SKILL_NAME",
        help="Cleanly remove an installed skill from the specified scope",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Replace existing target directory",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and print deploy plan without copying or deleting files",
    )
    args = parser.parse_args()

    data_dir = Path(args.data_dir).resolve() if args.data_dir else default_data_dir()
    workspace = Path(args.workspace).resolve() if args.workspace else None

    action_count = sum(bool(x) for x in (args.skill_source, args.update, args.remove))
    if action_count == 0:
        parser.error("Provide <skill_source>, --update <skill-name>, or --remove <skill-name>.")
    if action_count > 1:
        parser.error("Use only one of <skill_source>, --update, or --remove per invocation.")

    if args.remove:
        return handle_remove(
            skill_name_raw=args.remove,
            scope=args.scope,
            data_dir=data_dir,
            workspace=workspace,
            assistant_id=args.assistant_id,
            dry_run=args.dry_run,
        )

    if args.update:
        return handle_update(
            skill_name_raw=args.update,
            scope=args.scope,
            data_dir=data_dir,
            workspace=workspace,
            assistant_id=args.assistant_id,
            ref_override=args.ref,
            approve_scripts=args.approve_scripts,
            dry_run=args.dry_run,
            validate_skill_path=validate_skill_path,
            format_detailed_report=format_detailed_report,
            get_skill_name=get_skill_name,
        )

    if args.skill_source is None:
        parser.error("Provide <skill_source>.")
    raw_source = args.skill_source.strip()

    if raw_source.startswith("@"):
        print(
            f"[DEPLOY-SOURCE-001] ERROR: Skill source '{raw_source}' looks like a tag or alias.\n"
            "  Fix: Pass the directory path that contains SKILL.md or a https://github.com/... URL.",
            file=sys.stderr,
        )
        return 2

    if raw_source.startswith(("https://", "http://")) and not is_github_source(raw_source):
        print(
            f"[DEPLOY-GITHUB-001] ERROR: Only github.com URLs are supported (got '{raw_source}').\n"
            "  Fix: Pass a https://github.com/<owner>/<repo> URL or a local skill directory.",
            file=sys.stderr,
        )
        return 2

    if is_github_source(raw_source):
        if not args.scope and not args.inspect:
            print(
                "[DEPLOY-ARGS-005] ERROR: --scope is required when deploying from a GitHub URL (unless --inspect is used).\n"
                "  Fix: Pass --scope workspace (with --workspace <root>), --scope global, or --scope assistant.",
                file=sys.stderr,
            )
            return 2

        try:
            parsed = parse_github_url(
                raw_source,
                ref_override=args.ref,
                subpath_override=args.subpath,
            )
            spec = resolve_github_spec(parsed)
            with tempfile.TemporaryDirectory(prefix="skill-fetch-") as tmp:
                repo_root = fetch_github_repo_at_commit(spec, Path(tmp))
                selected_dir, skill_subpath = select_github_skill(
                    repo_root=repo_root,
                    spec=spec,
                    skill_filter=args.skill,
                    inspect_only=args.inspect,
                    validate_skill_path=validate_skill_path,
                    get_skill_name=get_skill_name,
                )

                source_report = run_strict_validation(
                    selected_dir,
                    "Pre-deploy validation (GitHub source)",
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

                fm = source_report.frontmatter if isinstance(source_report.frontmatter, dict) else {}
                desc = str(fm.get("description", "")).strip()
                scripts = scan_skill_scripts(selected_dir)

                if args.inspect and not args.scope:
                    print_review_summary(
                        skill_name=skill_name,
                        description=desc,
                        source_label=spec.repo_url,
                        scripts=scripts,
                        commit=spec.commit,
                        ref=spec.ref,
                        subpath=skill_subpath,
                    )
                    print("\nValidation: strict (pre-deploy passed)")
                    if scripts:
                        print(
                            "Action required before install: Ask the user to approve the scripts above, "
                            "then run deploy_skill.py with --scope <scope> --approve-scripts."
                        )
                    else:
                        print(
                            "Ready to install: Run deploy_skill.py with --scope <scope> to deploy."
                        )
                    return 0

                if args.scope is None:
                    print(
                        "[DEPLOY-ARGS-005] ERROR: --scope is required.\n"
                        "  Fix: Pass --scope workspace, --scope global, or --scope assistant.",
                        file=sys.stderr,
                    )
                    return 2

                if args.scope in {"global", "assistant"}:
                    assert_data_dir_exists(data_dir)
                target = resolve_target_dir(
                    args.scope,
                    skill_name,
                    data_dir,
                    workspace,
                    args.assistant_id,
                )
                assert_target_allowed(target)

                return execute_deploy_to_target(
                    source_dir=selected_dir,
                    target=target,
                    scope=args.scope,
                    skill_name=skill_name,
                    description=desc,
                    source_label=spec.repo_url,
                    scripts=scripts,
                    provenance_builder=lambda existing: build_github_provenance(
                        skill_name=skill_name,
                        spec=spec,
                        skill_subpath=skill_subpath,
                        scripts=scripts,
                        existing=existing,
                    ),
                    overwrite=args.overwrite,
                    dry_run=args.dry_run or args.inspect,
                    require_approval=True,
                    approve_scripts=args.approve_scripts,
                    validate_skill_path=validate_skill_path,
                    format_detailed_report=format_detailed_report,
                    commit=spec.commit,
                    ref=spec.ref,
                    subpath=skill_subpath,
                )
        except GitHubDeployError as exc:
            print(exc.format(), file=sys.stderr)
            return exc.exit_code

    # Local directory source
    source = Path(raw_source).resolve()
    if not source.is_dir():
        print(
            "[DEPLOY-SOURCE-001] ERROR: Skill source is not a directory.\n"
            f"  Path: {source}\n"
            "  Fix: Pass the folder that contains SKILL.md, or a https://github.com/... URL.",
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

    target_preview: Path | None = None
    if args.scope:
        if args.scope in {"global", "assistant"} and not args.inspect:
            assert_data_dir_exists(data_dir)
        prelim_report = validate_skill_path(source, strict=False)
        prelim_name = get_skill_name(prelim_report) or source.name
        if (args.scope != "workspace" or workspace is not None) and (
            args.scope != "assistant" or args.assistant_id
        ):
            target_preview = resolve_target_dir(
                args.scope, prelim_name, data_dir, workspace, args.assistant_id
            )

    allow_stripped_local = target_preview is None or source.resolve() != target_preview.resolve()
    source_report = run_strict_validation(
        source,
        "Pre-deploy validation (source)",
        validate_skill_path,
        format_detailed_report,
        allow_stripped_root_files=allow_stripped_local,
    )
    skill_name = get_skill_name(source_report)
    if not skill_name:
        print(
            "[DEPLOY-SOURCE-003] ERROR: Could not read skill name from validated frontmatter.\n"
            "  Fix: Ensure frontmatter contains name: <folder-name>.",
            file=sys.stderr,
        )
        return 1

    fm = source_report.frontmatter if isinstance(source_report.frontmatter, dict) else {}
    desc = str(fm.get("description", "")).strip()
    scripts = scan_skill_scripts(source)

    if args.inspect and not args.scope:
        print_review_summary(
            skill_name=skill_name,
            description=desc,
            source_label=str(source),
            scripts=scripts,
        )
        print("\nValidation: strict (pre-deploy passed)")
        return 0

    if not args.scope:
        print(
            "[DEPLOY-ARGS-005] ERROR: --scope is required.\n"
            "  Fix: Pass --scope workspace (with --workspace <root>), --scope global, or --scope assistant.",
            file=sys.stderr,
        )
        return 2

    assert_data_dir_exists(data_dir)
    target = resolve_target_dir(
        args.scope,
        skill_name,
        data_dir,
        workspace,
        args.assistant_id,
    )
    assert_target_allowed(target)

    return execute_deploy_to_target(
        source_dir=source,
        target=target,
        scope=args.scope,
        skill_name=skill_name,
        description=desc,
        source_label=str(source),
        scripts=scripts,
        provenance_builder=lambda existing: build_local_provenance(
            skill_name, source, scripts, existing
        ),
        overwrite=args.overwrite,
        dry_run=args.dry_run or args.inspect,
        require_approval=False,
        approve_scripts=args.approve_scripts,
        validate_skill_path=validate_skill_path,
        format_detailed_report=format_detailed_report,
    )


if __name__ == "__main__":
    raise SystemExit(main())
