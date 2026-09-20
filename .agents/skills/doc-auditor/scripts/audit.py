#!/usr/bin/env python3
"""
Doc Auditor — Analyze codebase structure against documentation to find gaps.

Usage:
    python3 audit.py <root-dir> [--output <json-path>] [--verbose]

Outputs a JSON report mapping discovered modules/features to their documentation status.
"""

import json
import os
import sys
import argparse
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional


# ── Data model ──────────────────────────────────────────────────────────────

@dataclass
class Module:
    path: str
    normalized_name: str
    kind: str          # "frontend-feature" | "frontend-lib" | "backend-module" | "benchmark"
    has_docs: bool = False
    has_user_docs: bool = False
    has_arch_docs: bool = False
    doc_paths: list = field(default_factory=list)
    gap: bool = False
    priority: str = "P3"  # P0..P3 or "Covered"


@dataclass
class Report:
    root: str
    readme_files: list = field(default_factory=list)
    docs_structure: dict = field(default_factory=dict)
    frontend_features: list = field(default_factory=list)
    frontend_libs: list = field(default_factory=list)
    backend_modules: list = field(default_factory=list)
    benchmark_dirs: list = field(default_factory=list)
    gaps: list = field(default_factory=list)
    summary: dict = field(default_factory=dict)


# ── Known patterns ──────────────────────────────────────────────────────────

# Features/services considered user-facing (kebab-case normalized)
USER_FACING_FEATURES = {
    # Frontend features
    "recipes", "scheduled-tasks", "migration", "session-export", "search",
    "settings", "history", "mcp-servers",
    # Backend services that power user features
    "browser-sidecar", "scheduled", "search",
    "lifecycle", "session-isolation", "session",
    # UI/systems features
    "push-notify", "message-context", "session-cancel", "tool-loop", "text-loop",
    "soul-lounge", "workflow-settlement", "type-safety",
    "rust-conventions", "message-streaming", "workspace-sync",
    "logger", "i18n", "media-settings", "retry",
    "llm-config", "ai-service", "analytics",
    "skill-creator", "skill-deployer", "skill-proposer",
    "teamwork", "org", "org-restructure",
    "email-integration", "calendar-mgmt", "telegram-cli", "ig-cli",
    "x-cli", "comfyui-generate", "persona-daily-schedule",
}

# System/architecture modules where spec/architecture docs are valuable (P2)
SPEC_OR_SYSTEM_KEYWORDS = {
    "schema", "validator", "convention", "mode", "protocol", "policy", "lifecycle"
}

# Internal-only modules/files to skip or classify as low priority (P3)
INTERNAL_EXACT = {
    "__tests__", "--tests--", "__pycache__", "node_modules", ".vitepress", "generated",
    "test", "perf", "performance", "benchmark",
    "utils", "config", "state", "models", "entity", "repositories",
    "commands", "server", "services", "mcp", "mcp.rs", "logger.rs",
    "lib.rs", "main.rs", "db", "schemas", "session-id", "session-metadata",
    "chat-utils", "date-utils", "file-url", "message-utils", "mime-utils",
    "tool-call-utils", "notify-file-download", "session-utils",
    "logger-config", "logger-core", "logger-file-manager", "logger-queue",
    "logger-utils", "logger.ts", "assistant", "backend", "vite-env.d.ts",
}


# ── Helpers ─────────────────────────────────────────────────────────────────

def normalize_name(raw_path: str) -> str:
    """Normalize file/dir path to a clean kebab-case name.
    
    Examples:
        "src/features/scheduled-tasks/" -> "scheduled-tasks"
        "src-tauri/src/browser_sidecar/" -> "browser-sidecar"
        "src-tauri/src/db_schema_validator.rs" -> "db-schema-validator"
        "lib/media-settings.ts" -> "media-settings"
    """
    p = raw_path.strip("/")
    # Take the last path component
    basename = os.path.basename(p)
    # Strip extension
    name = os.path.splitext(basename)[0]
    # Replace underscore with hyphen for cross-language matching (Rust snake_case -> kebab-case)
    return name.replace("_", "-").lower()


# ── Discovery ───────────────────────────────────────────────────────────────

def discover_readme(root: Path) -> list[str]:
    """Find all README files in the repo."""
    results = []
    for p in root.rglob("README*"):
        if p.is_file() and "node_modules" not in p.parts:
            results.append(str(p.relative_to(root)))
    return sorted(results)


def discover_docs_structure(root: Path) -> dict:
    """Map the docs/ directory tree."""
    docs_dir = root / "docs"
    if not docs_dir.is_dir():
        return {}
    result = {}
    for p in docs_dir.rglob("*"):
        if p.is_file() and p.suffix == ".md":
            rel = str(p.relative_to(docs_dir))
            parent = str(p.relative_to(docs_dir).parent)
            if parent not in result:
                result[parent] = []
            result[parent].append(rel)
    return result


def discover_frontend_features(root: Path) -> list[str]:
    """List frontend feature directories."""
    features_dir = root / "src" / "features"
    if not features_dir.is_dir():
        return []
    return sorted([d.name for d in features_dir.iterdir() if d.is_dir()])


def discover_frontend_libs(root: Path) -> list[str]:
    """List frontend lib modules and files."""
    lib_dir = root / "src" / "lib"
    if not lib_dir.is_dir():
        return []
    items = []
    for d in sorted(lib_dir.iterdir()):
        if d.is_dir():
            items.append(f"lib/{d.name}/")
        elif d.suffix == ".ts" and not d.name.startswith("__"):
            items.append(f"lib/{d.name}")
    return items


def discover_backend_modules(root: Path) -> list[str]:
    """List backend source modules."""
    src = root / "src-tauri" / "src"
    if not src.is_dir():
        return []
    items = []
    for d in sorted(src.iterdir()):
        if d.is_dir():
            items.append(f"src-tauri/src/{d.name}/")
        elif d.suffix == ".rs" and d.name not in ("main.rs", "lib.rs"):
            items.append(f"src-tauri/src/{d.name}")
    return items


def discover_benchmarks(root: Path) -> list[str]:
    """Find benchmark directories."""
    results = []
    for d in root.iterdir():
        if d.is_dir() and ("bench" in d.name.lower() or "benchmark" in d.name.lower()):
            results.append(d.name)
    return sorted(results)


def has_docs_for(norm_name: str, docs_structure: dict) -> tuple[bool, bool, list[str]]:
    """Check if a module has corresponding documentation.
    
    Returns:
        (has_user_docs, has_arch_docs, matching_doc_paths)
    """
    matched_paths = []
    has_user_docs = False
    has_arch_docs = False

    name_tokens = set(norm_name.split("-"))

    for parent, files in docs_structure.items():
        is_user_dir = "user" in parent.lower() or "guide" in parent.lower()
        is_arch_dir = any(k in parent.lower() for k in ["architecture", "spec", "feature", "api", "reference"])

        for f in files:
            doc_stem = Path(f).stem.lower().replace("_", "-")
            doc_tokens = set(doc_stem.split("-"))

            # Matching criteria:
            # 1. Exact match (e.g. "recipes" == "recipes", "scheduled-tasks" == "scheduled-tasks")
            # 2. Complete token subset if multi-token (e.g. "browser-sidecar" in "browser-sidecar-guide")
            is_match = False
            if norm_name == doc_stem:
                is_match = True
            elif len(norm_name) >= 4 and norm_name in doc_stem:
                is_match = True
            elif len(norm_name) >= 4 and doc_stem in norm_name and len(doc_stem) >= 4:
                is_match = True
            elif len(name_tokens) > 1 and name_tokens.issubset(doc_tokens):
                is_match = True

            if is_match:
                doc_rel = f"docs/{f}"
                if doc_rel not in matched_paths:
                    matched_paths.append(doc_rel)
                if is_user_dir:
                    has_user_docs = True
                if is_arch_dir:
                    has_arch_docs = True

    return has_user_docs, has_arch_docs, matched_paths


def classify_priority(norm_name: str, kind: str, has_user_docs: bool, has_arch_docs: bool) -> tuple[str, bool]:
    """Classify documentation priority and determine if it represents a gap.
    
    Returns:
        (priority: "P0"|"P1"|"P2"|"P3"|"Covered", is_gap: bool)
    """
    # 1. Internal modules are always low priority (P3, not a critical gap)
    if norm_name in INTERNAL_EXACT or any(p in norm_name for p in ["__test", "generated", "tmp-"]):
        return ("P3", False)

    is_user_facing = (
        norm_name in USER_FACING_FEATURES or
        kind == "frontend-feature"
    )

    # 2. User-facing features
    if is_user_facing:
        if not has_user_docs and not has_arch_docs:
            return ("P0", True)  # Zero docs for user-facing feature
        if not has_user_docs and has_arch_docs:
            return ("P1", True)  # Documented internally/arch, but missing user-facing guide
        # Has user docs
        return ("Covered", False)

    # 3. System / Spec / Architecture modules
    is_spec_or_system = any(kw in norm_name for kw in SPEC_OR_SYSTEM_KEYWORDS)
    if is_spec_or_system:
        if not has_arch_docs and not has_user_docs:
            return ("P2", True)  # Architecture/spec docs missing
        return ("Covered", False)

    # 4. Other general code modules
    has_any_docs = has_user_docs or has_arch_docs
    if not has_any_docs:
        return ("P3", True)

    return ("Covered", False)


# ── Audit Execution ─────────────────────────────────────────────────────────

def audit(root_str: str, output_str: Optional[str] = None, verbose: bool = False) -> Report:
    root = Path(root_str).resolve()

    readme_files = discover_readme(root)
    docs_structure = discover_docs_structure(root)
    frontend_features = discover_frontend_features(root)
    frontend_libs = discover_frontend_libs(root)
    backend_modules = discover_backend_modules(root)
    benchmark_dirs = discover_benchmarks(root)

    report = Report(
        root=str(root),
        readme_files=readme_files,
        docs_structure=docs_structure,
    )

    # Analyze frontend features
    for feat in frontend_features:
        norm = normalize_name(feat)
        has_user, has_arch, doc_paths = has_docs_for(norm, docs_structure)
        has_docs = len(doc_paths) > 0
        priority, gap = classify_priority(norm, "frontend-feature", has_user, has_arch)

        mod = Module(
            path=f"src/features/{feat}/",
            normalized_name=norm,
            kind="frontend-feature",
            has_docs=has_docs,
            has_user_docs=has_user,
            has_arch_docs=has_arch,
            doc_paths=doc_paths,
            gap=gap,
            priority=priority,
        )
        report.frontend_features.append(mod)
        if mod.gap:
            report.gaps.append(asdict(mod))
        if verbose:
            print(f"  [FEAT] {feat:25s} -> {norm:20s} docs={has_docs} (u={has_user}, a={has_arch}) p={priority}")

    # Analyze frontend libs
    for lib in frontend_libs:
        norm = normalize_name(lib)
        has_user, has_arch, doc_paths = has_docs_for(norm, docs_structure)
        has_docs = len(doc_paths) > 0
        priority, gap = classify_priority(norm, "frontend-lib", has_user, has_arch)

        mod = Module(
            path=lib,
            normalized_name=norm,
            kind="frontend-lib",
            has_docs=has_docs,
            has_user_docs=has_user,
            has_arch_docs=has_arch,
            doc_paths=doc_paths,
            gap=gap,
            priority=priority,
        )
        report.frontend_libs.append(mod)
        if mod.gap:
            report.gaps.append(asdict(mod))

    # Analyze backend modules
    for mod_name in backend_modules:
        norm = normalize_name(mod_name)
        has_user, has_arch, doc_paths = has_docs_for(norm, docs_structure)
        has_docs = len(doc_paths) > 0
        priority, gap = classify_priority(norm, "backend-module", has_user, has_arch)

        mod = Module(
            path=mod_name,
            normalized_name=norm,
            kind="backend-module",
            has_docs=has_docs,
            has_user_docs=has_user,
            has_arch_docs=has_arch,
            doc_paths=doc_paths,
            gap=gap,
            priority=priority,
        )
        report.backend_modules.append(mod)
        if mod.gap:
            report.gaps.append(asdict(mod))
        if verbose:
            print(f"  [RUST] {mod_name:30s} -> {norm:20s} docs={has_docs} (u={has_user}, a={has_arch}) p={priority}")

    # Benchmarks
    for bm in benchmark_dirs:
        bm_path = root / bm
        bm_docs = list(bm_path.rglob("README*"))
        has_docs = len(bm_docs) > 0
        priority = "Covered" if has_docs else "P2"
        gap = not has_docs
        mod = Module(
            path=bm,
            normalized_name=bm,
            kind="benchmark",
            has_docs=has_docs,
            has_user_docs=False,
            has_arch_docs=has_docs,
            doc_paths=[str(p.relative_to(root)) for p in bm_docs],
            gap=gap,
            priority=priority,
        )
        report.benchmark_dirs.append(asdict(mod))
        if gap:
            report.gaps.append(asdict(mod))

    # Summary
    total_modules = len(report.frontend_features) + len(report.frontend_libs) + len(report.backend_modules)
    total_gaps = len(report.gaps)
    by_priority = {}
    for g in report.gaps:
        by_priority[g["priority"]] = by_priority.get(g["priority"], 0) + 1

    report.summary = {
        "total_modules_scanned": total_modules,
        "total_gaps": total_gaps,
        "coverage_pct": round((1 - total_gaps / max(total_modules, 1)) * 100, 1),
        "gaps_by_priority": by_priority,
    }

    return report


def main():
    parser = argparse.ArgumentParser(description="Doc Auditor: Find documentation gaps in a codebase")
    parser.add_argument("root", help="Root directory of the codebase")
    parser.add_argument("--output", "-o", help="Output JSON file path (default: stdout)")
    parser.add_argument("--verbose", "-v", action="store_true", help="Print per-module status")
    args = parser.parse_args()

    if args.verbose:
        print(f"Auditing: {args.root}")

    report = audit(args.root, output_str=args.output, verbose=args.verbose)
    data = asdict(report)

    if args.output:
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        print(f"Report written to: {args.output}")
    else:
        print(json.dumps(data, indent=2, ensure_ascii=False))

    # Print summary to stderr/stdout
    print(f"\n=== Summary ===", file=sys.stderr)
    print(f"Modules scanned: {report.summary['total_modules_scanned']}", file=sys.stderr)
    print(f"Gaps found:      {report.summary['total_gaps']}", file=sys.stderr)
    print(f"Coverage:        {report.summary['coverage_pct']}%", file=sys.stderr)
    if report.summary["gaps_by_priority"]:
        print(f"By priority:", file=sys.stderr)
        for p in ["P0", "P1", "P2", "P3"]:
            if p in report.summary["gaps_by_priority"]:
                print(f"  {p}: {report.summary['gaps_by_priority'][p]}", file=sys.stderr)


if __name__ == "__main__":
    main()
