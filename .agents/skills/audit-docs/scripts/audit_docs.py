#!/usr/bin/env python3
"""
audit_docs.py - Audit documentation gaps between the baseline commit and HEAD.

Identifies code changes (features, tools, skills, settings) made since the
last docs update (or a specified git ref/release tag) and checks whether
user-facing documentation (KO and EN in docs/user/) reflects those changes.
Also checks KO <-> EN documentation parity.
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path


def run_git(args, cwd=None):
    """Run git command and return stripped stdout."""
    res = subprocess.run(
        ["git"] + args,
        cwd=cwd,
        capture_output=True,
        text=True,
        check=True,
    )
    return res.stdout.strip()


def find_last_docs_commit(workspace_root):
    """Find the most recent commit that touched docs/user/."""
    try:
        commit_hash = run_git(
            ["log", "-n", "1", "--format=%H", "--", "docs/user/"],
            cwd=workspace_root,
        )
        commit_summary = run_git(
            ["log", "-n", "1", "--format=%h %s (%cd)", "--date=short", "--", "docs/user/"],
            cwd=workspace_root,
        )
        return commit_hash, commit_summary
    except subprocess.CalledProcessError:
        return None, None


def find_last_release_tag(workspace_root):
    """Find the most recent release tag."""
    try:
        tag = run_git(["describe", "--tags", "--abbrev=0"], cwd=workspace_root)
        commit_summary = run_git(
            ["log", "-n", "1", "--format=%h %s (%cd)", "--date=short", tag],
            cwd=workspace_root,
        )
        return tag, commit_summary
    except subprocess.CalledProcessError:
        return None, None


def get_changed_files(workspace_root, base_ref, head_ref="HEAD"):
    """Get list of changed files with their status (A, M, D, R, etc.)."""
    try:
        output = run_git(
            ["diff", "--name-status", f"{base_ref}..{head_ref}"],
            cwd=workspace_root,
        )
        changes = []
        for line in output.splitlines():
            if not line.strip():
                continue
            parts = line.split(maxsplit=1)
            if len(parts) == 2:
                changes.append((parts[0], parts[1]))
        return changes
    except subprocess.CalledProcessError as e:
        print(f"Error running git diff: {e}", file=sys.stderr)
        return []


def categorize_code_changes(changes):
    """Group changed files by architectural area."""
    categories = {
        "skills": [],
        "builtin_tools": [],
        "ui_views_and_routes": [],
        "settings_and_config": [],
        "backend_core": [],
        "docs_and_website": [],
        "other": [],
    }

    for status, filepath in changes:
        entry = {"status": status, "path": filepath}
        if "bundled_skills/" in filepath or ".agents/skills/" in filepath:
            categories["skills"].append(entry)
        elif "src-tauri/src/mcp/" in filepath or "src-tauri/src/agent/tools" in filepath:
            categories["builtin_tools"].append(entry)
        elif filepath.startswith("src/views/") or filepath.startswith("src/components/"):
            if "settings" in filepath:
                categories["settings_and_config"].append(entry)
            else:
                categories["ui_views_and_routes"].append(entry)
        elif "config" in filepath or "settings" in filepath:
            categories["settings_and_config"].append(entry)
        elif filepath.startswith("docs/user/") or filepath.startswith("website/"):
            categories["docs_and_website"].append(entry)
        elif filepath.startswith("src-tauri/"):
            categories["backend_core"].append(entry)
        else:
            categories["other"].append(entry)

    return categories


def check_ko_en_parity(workspace_root):
    """Check parity between Korean and English user documentation."""
    docs_user = Path(workspace_root) / "docs" / "user"
    en_docs_user = docs_user / "en"

    if not docs_user.exists():
        return []

    results = []

    # Iterate through all .md files in docs/user (excluding en/ and READMEs)
    for ko_file in docs_user.rglob("*.md"):
        rel_to_user = ko_file.relative_to(docs_user)
        parts = rel_to_user.parts
        if parts[0] == "en" or ko_file.name == "README.md":
            continue

        en_file = en_docs_user / rel_to_user
        ko_size = ko_file.stat().st_size
        ko_lines = len(ko_file.read_text(encoding="utf-8", errors="ignore").splitlines())

        if not en_file.exists():
            results.append({
                "path": str(rel_to_user),
                "ko_size": ko_size,
                "ko_lines": ko_lines,
                "en_size": 0,
                "en_lines": 0,
                "status": "MISSING_EN",
                "ratio": 0.0,
            })
        else:
            en_size = en_file.stat().st_size
            en_lines = len(en_file.read_text(encoding="utf-8", errors="ignore").splitlines())
            ratio = en_size / ko_size if ko_size > 0 else 1.0

            status = "OK"
            if ratio < 0.4:
                status = "SEVERE_TRUNCATION"
            elif ratio < 0.7:
                status = "MODERATE_TRUNCATION"

            results.append({
                "path": str(rel_to_user),
                "ko_size": ko_size,
                "ko_lines": ko_lines,
                "en_size": en_size,
                "en_lines": en_lines,
                "status": status,
                "ratio": round(ratio, 2),
            })

    return results


def analyze_potential_gaps(categories, parity_list):
    """Determine potential undocumented changes based on code modifications."""
    gaps = []

    # 1. New or modified skills
    modified_skills = set()
    for item in categories["skills"]:
        m = re.search(r"(?:bundled_skills|\.agents/skills)/([^/]+)", item["path"])
        if m:
            modified_skills.add(m.group(1))

    for skill in sorted(modified_skills):
        gaps.append({
            "type": "SKILL_CHANGE",
            "item": skill,
            "description": f"Skill '{skill}' modified in code. Check guides/skills.md.",
            "target_doc": "guides/skills.md",
        })

    # 2. Builtin tools
    tool_changes = [item["path"] for item in categories["builtin_tools"]]
    if tool_changes:
        gaps.append({
            "type": "TOOL_CHANGE",
            "item": f"{len(tool_changes)} tool-related files",
            "description": "Backend tool implementations modified. Check guides/builtin-tools.md.",
            "target_doc": "guides/builtin-tools.md",
        })

    # 3. Settings / Config changes
    settings_changes = [item["path"] for item in categories["settings_and_config"]]
    if settings_changes:
        gaps.append({
            "type": "CONFIG_CHANGE",
            "item": f"{len(settings_changes)} setting/config files",
            "description": "Settings UI or configuration files changed. Check guides/troubleshooting.md and getting-started/.",
            "target_doc": "guides/troubleshooting.md",
        })

    # 4. Parity gaps
    for p in parity_list:
        if p["status"] in ("MISSING_EN", "SEVERE_TRUNCATION"):
            gaps.append({
                "type": "PARITY_GAP",
                "item": p["path"],
                "description": f"English doc is {p['status']} (ratio: {int(p['ratio']*100)}% of KO).",
                "target_doc": f"en/{p['path']}",
            })

    return gaps


def check_stk_compliance(workspace_root):
    """Lint Korean user docs against Simplified Technical Korean (STK / ASD-STE100 equivalent) rules."""
    docs_user = Path(workspace_root) / "docs" / "user"
    findings = []

    # Regex patterns for STK violations
    patterns = [
        (re.compile(r"되어지[다면는]|이루어지[다면는]"), "이중 피동 표현 (능동태 또는 단순 피동 권장)"),
        (re.compile(r"[에의]\s*대하[여면인]"), "번역투 표현 (~에 대하여 대신 목적격 조사나 간결한 표현 권장)"),
        (re.compile(r"[을를]\s*통하[여면인]"), "불필요한 매개 번역투 표현 (~을 통하여 대신 ~로/~사용하여 권장)"),
        (re.compile(r"하는\s*것이\s*가능"), "장황한 가능 표현 (~할 수 있습니다 권장)"),
        (re.compile(r"[을를]\s*필요로\s*한"), "영어 require 직역투 (~해야 합니다/~가 필요합니다 권장)"),
    ]

    for ko_file in sorted(docs_user.rglob("*.md")):
        rel_to_user = ko_file.relative_to(docs_user)
        if rel_to_user.parts[0] == "en" or ko_file.name == "README.md":
            continue

        text = ko_file.read_text(encoding="utf-8", errors="ignore")
        for line_no, line in enumerate(text.splitlines(), start=1):
            stripped = line.strip()
            # Skip code blocks and headings
            if stripped.startswith("```") or stripped.startswith("#") or stripped.startswith("|"):
                continue

            for pattern, reason in patterns:
                m = pattern.search(stripped)
                if m:
                    findings.append({
                        "file": str(rel_to_user),
                        "line": line_no,
                        "match": m.group(0),
                        "snippet": stripped[:60] + ("..." if len(stripped) > 60 else ""),
                        "reason": reason,
                    })

            # Check overly long sentences (over 25 words in a single sentence)
            words = stripped.split()
            if len(words) > 28 and not stripped.startswith("-"):
                findings.append({
                    "file": str(rel_to_user),
                    "line": line_no,
                    "match": f"{len(words)} words",
                    "snippet": stripped[:60] + "...",
                    "reason": "문장 길이 초과 (STK 지침: 설명문 20~25어절 이하 단문 권장)",
                })

    return findings


def generate_markdown_report(base_info, head_ref, categories, parity_list, gaps, stk_findings):
    """Format audit results as a readable markdown document."""
    lines = []
    lines.append("# 📋 Documentation Gap Audit Report\n")
    lines.append(f"- **Baseline Commit**: `{base_info['hash']}` ({base_info['summary']})")
    lines.append(f"- **Target Head**: `{head_ref}`")
    lines.append(f"- **Baseline Mode**: {base_info['mode']}\n")

    lines.append("## 1. Code Changes Summary Since Baseline\n")
    lines.append("| Category | Changed Files Count |")
    lines.append("| --- | --- |")
    for cat, items in categories.items():
        if cat != "docs_and_website":
            lines.append(f"| {cat.replace('_', ' ').title()} | {len(items)} |")
    lines.append(f"| Docs & Website Updates | {len(categories['docs_and_website'])} |")
    lines.append("")

    lines.append("## 2. Potential Documentation Gaps\n")
    if not gaps:
        lines.append("✅ No documentation gaps detected. Documentation is aligned with code changes.\n")
    else:
        lines.append("| Gap Type | Target Item | Suggested Action | Target Document |")
        lines.append("| --- | --- | --- | --- |")
        for g in gaps:
            lines.append(f"| `{g['type']}` | {g['item']} | {g['description']} | `{g['target_doc']}` |")
        lines.append("")

    lines.append("## 3. KO ↔ EN Documentation Parity Audit\n")
    lines.append("| Document Path | KO Lines / Size | EN Lines / Size | Ratio (EN/KO) | Status |")
    lines.append("| --- | --- | --- | --- | --- |")
    for p in parity_list:
        ko_str = f"{p['ko_lines']}L / {p['ko_size']}B"
        en_str = f"{p['en_lines']}L / {p['en_size']}B" if p['en_size'] > 0 else "Missing"
        ratio_pct = f"{int(p['ratio'] * 100)}%"
        icon = "✅" if p['status'] == "OK" else ("⚠️" if p['status'] == "MODERATE_TRUNCATION" else "🔴")
        lines.append(f"| `{p['path']}` | {ko_str} | {en_str} | {ratio_pct} | {icon} {p['status']} |")
    lines.append("")

    lines.append("## 4. Simplified Technical Korean (STK / ASD-STE100) Style Audit\n")
    if not stk_findings:
        lines.append("✅ All Korean documents adhere cleanly to STK / ASD-STE100 guidelines.\n")
    else:
        lines.append(f"Found {len(stk_findings)} potential style improvements (passive voice, translation patterns, long sentences):\n")
        lines.append("| File | Line | Matched Text | Issue / Guideline |")
        lines.append("| --- | --- | --- | --- |")
        for f in stk_findings[:20]:  # Cap at 20 items for readability
            lines.append(f"| `{f['file']}` | L{f['line']} | `{f['match']}` | {f['reason']} |")
        if len(stk_findings) > 20:
            lines.append(f"\n*(Showing top 20 of {len(stk_findings)} total findings)*\n")
        lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Audit documentation gaps against git commits.")
    parser.add_argument("--since", help="Base git commit, branch, or tag (defaults to last commit touching docs/user/)")
    parser.add_argument("--release", action="store_true", help="Use latest release tag as baseline")
    parser.add_argument("--head", default="HEAD", help="Head commit or branch to compare (default: HEAD)")
    parser.add_argument("--json", action="store_true", help="Output audit report in JSON format")
    parser.add_argument("--output", help="Write report to output file")
    args = parser.parse_args()

    workspace_root = os.getcwd()

    # Determine baseline
    base_hash = None
    base_summary = None
    mode = "Custom Ref"

    if args.since:
        base_hash = args.since
        try:
            base_summary = run_git(["log", "-n", "1", "--format=%h %s (%cd)", "--date=short", base_hash], cwd=workspace_root)
        except Exception:
            base_summary = base_hash
        mode = f"Specified (--since {args.since})"
    elif args.release:
        base_hash, base_summary = find_last_release_tag(workspace_root)
        mode = "Latest Release Tag"
    else:
        # Default: last commit touching docs/user/
        base_hash, base_summary = find_last_docs_commit(workspace_root)
        mode = "Last Commit Touching docs/user/"

    if not base_hash:
        print("Error: Could not determine baseline commit.", file=sys.stderr)
        sys.exit(1)

    base_info = {
        "hash": base_hash,
        "summary": base_summary or base_hash,
        "mode": mode,
    }

    changes = get_changed_files(workspace_root, base_hash, args.head)
    categories = categorize_code_changes(changes)
    parity_list = check_ko_en_parity(workspace_root)
    gaps = analyze_potential_gaps(categories, parity_list)
    stk_findings = check_stk_compliance(workspace_root)

    if args.json:
        report_data = {
            "baseline": base_info,
            "head": args.head,
            "categories": {k: len(v) for k, v in categories.items()},
            "gaps": gaps,
            "parity": parity_list,
            "stk_findings": stk_findings,
        }
        report_text = json.dumps(report_data, indent=2, ensure_ascii=False)
    else:
        report_text = generate_markdown_report(base_info, args.head, categories, parity_list, gaps, stk_findings)

    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(report_text)
        print(f"Report written to {args.output}")
    else:
        print(report_text)


if __name__ == "__main__":
    main()
