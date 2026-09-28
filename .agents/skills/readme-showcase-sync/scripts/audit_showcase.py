#!/usr/bin/env python3
"""
Audit Showcase — Detect missing features and updates in Main README and Project Website.

Compares actual codebase features (frontend features, bundled skills, backend services)
against:
1. Main README (README.md, README.ko.md)
2. Project Page Landing (docs/user/index.md, docs/user/en/index.md)
3. VitePress Navigation (website/.vitepress/config.ts)

Usage:
    python3 audit_showcase.py <root-dir> [--output <json-path>] [--verbose]
"""

import json
import os
import sys
import argparse
import re
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional


# ── Data model ──────────────────────────────────────────────────────────────

@dataclass
class FeatureItem:
    key: str
    label: str
    kind: str  # "frontend-feature" | "bundled-skill" | "backend-service"
    in_readme: bool = False
    in_readme_ko: bool = False
    in_website_landing: bool = False
    in_website_sidebar: bool = False
    details: dict = field(default_factory=dict)


@dataclass
class ShowcaseAuditReport:
    total_features: int = 0
    missing_in_readme: list = field(default_factory=list)
    missing_in_readme_ko: list = field(default_factory=list)
    missing_in_landing: list = field(default_factory=list)
    missing_in_sidebar: list = field(default_factory=list)
    features: list = field(default_factory=list)


# ── Known search keywords for each feature ───────────────────────────────────

FEATURE_KEYWORDS = {
    # Key: [regex or terms to look for in markdown/config]
    "recipes": ["recipe", "솔루션 레시피", "morning briefing"],
    "scheduled-tasks": ["scheduled task", "cron", "예약 작업", "자동화", "pc-health-audit"],
    "browser-sidecar": ["browser-sidecar", "browser automation", "playwright", "브라우저 자동화", "sidecar"],
    "session-export": ["session-export", "session export", "atif", "세션 내보내기", "export format"],
    "mcp-servers": ["mcp server", "mcp-server", "extensions", "커스텀 mcp", "builtin-tools"],
    "history": ["history", "히스토리", "대화 기록", "session list"],
    "settings": ["settings", "설정", "provider api key", "ai & models"],
    "knowledge": ["knowledge", "지식", "rag", "vector", "bm25"],
    "migration": ["migration", "마이그레이션", "v4", "id migration"],
    "ig-cli": ["ig-cli", "instagram", "인스타그램"],
    "x-cli": ["x-cli", "twitter", "트위터", "twikit"],
    "telegram-cli": ["telegram-cli", "telegram", "텔레그램"],
    "comfyui-generate": ["comfyui", "image generate", "이미지 생성"],
    "session-isolation": ["session isolation", "세션 격리", "mcpserviceproxy"],
    "soul-lounge": ["soul-lounge", "soul lounge", "소울 라운지", "recovery loop"],
    "compact-planning": ["compact planning", "압축 플래닝", "compaction"],
}


# ── Scanners ────────────────────────────────────────────────────────────────

def discover_features(root: Path) -> list[FeatureItem]:
    items = []

    # 1. Frontend features
    feat_dir = root / "src" / "features"
    if feat_dir.is_dir():
        for d in sorted(feat_dir.iterdir()):
            if d.is_dir() and not d.name.startswith("__"):
                k = d.name.replace("_", "-").lower()
                items.append(FeatureItem(
                    key=k,
                    label=f"Frontend: {d.name}",
                    kind="frontend-feature"
                ))

    # 2. Bundled skills
    skills_dir = root / "src-tauri" / "bundled_skills"
    if skills_dir.is_dir():
        for d in sorted(skills_dir.iterdir()):
            if d.is_dir() and not d.name.startswith("."):
                k = d.name.replace("_", "-").lower()
                items.append(FeatureItem(
                    key=k,
                    label=f"Bundled Skill: {d.name}",
                    kind="bundled-skill"
                ))

    # 3. Core backend services
    backend_dir = root / "src-tauri" / "src"
    services = ["browser_sidecar", "session_export", "scheduled", "session_isolation"]
    for s in services:
        if (backend_dir / s).is_dir():
            k = s.replace("_", "-").lower()
            if not any(i.key == k for i in items):
                items.append(FeatureItem(
                    key=k,
                    label=f"Backend Service: {s}",
                    kind="backend-service"
                ))

    return items


def check_content_contains(content: str, feature_key: str) -> bool:
    content_lower = content.lower()
    keywords = FEATURE_KEYWORDS.get(feature_key, [feature_key.replace("-", " "), feature_key])
    for kw in keywords:
        pattern = r"\b" + re.escape(kw) + r"\b"
        if re.search(pattern, content_lower):
            return True
    return False


# ── Main Audit Logic ────────────────────────────────────────────────────────

def audit_showcase(root_str: str) -> ShowcaseAuditReport:
    root = Path(root_str).resolve()
    features = discover_features(root)

    # Read files
    readme_path = root / "README.md"
    readme_ko_path = root / "README.ko.md"
    landing_ko_path = root / "docs" / "user" / "index.md"
    landing_en_path = root / "docs" / "user" / "en" / "index.md"
    vitepress_config_path = root / "website" / ".vitepress" / "config.ts"

    readme_content = readme_path.read_text(encoding="utf-8") if readme_path.is_file() else ""
    readme_ko_content = readme_ko_path.read_text(encoding="utf-8") if readme_ko_path.is_file() else ""
    landing_ko_content = landing_ko_path.read_text(encoding="utf-8") if landing_ko_path.is_file() else ""
    landing_en_content = landing_en_path.read_text(encoding="utf-8") if landing_en_path.is_file() else ""
    vp_config_content = vitepress_config_path.read_text(encoding="utf-8") if vitepress_config_path.is_file() else ""

    landing_combined = f"{landing_ko_content}\n{landing_en_content}"

    report = ShowcaseAuditReport(total_features=len(features))

    for feat in features:
        feat.in_readme = check_content_contains(readme_content, feat.key)
        feat.in_readme_ko = check_content_contains(readme_ko_content, feat.key)
        feat.in_website_landing = check_content_contains(landing_combined, feat.key)
        feat.in_website_sidebar = check_content_contains(vp_config_content, feat.key)

        feat_dict = asdict(feat)
        report.features.append(feat_dict)

        if not feat.in_readme:
            report.missing_in_readme.append(feat.key)
        if not feat.in_readme_ko and readme_ko_content:
            report.missing_in_readme_ko.append(feat.key)
        if not feat.in_website_landing:
            report.missing_in_landing.append(feat.key)
        if not feat.in_website_sidebar:
            report.missing_in_sidebar.append(feat.key)

    return report


def main():
    parser = argparse.ArgumentParser(description="Audit Main README and Project Website for missing features")
    parser.add_argument("root", help="Root directory of repository")
    parser.add_argument("--output", "-o", help="Output JSON path")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    args = parser.parse_args()

    report = audit_showcase(args.root)
    data = asdict(report)

    if args.output:
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        print(f"Report written to {args.output}")
    else:
        print(json.dumps(data, indent=2, ensure_ascii=False))

    print("\n=== Showcase Audit Summary ===", file=sys.stderr)
    print(f"Total Features Scanned: {report.total_features}", file=sys.stderr)
    print(f"Missing in Main README: {len(report.missing_in_readme)} -> {', '.join(report.missing_in_readme)}", file=sys.stderr)
    print(f"Missing in README.ko:   {len(report.missing_in_readme_ko)} -> {', '.join(report.missing_in_readme_ko)}", file=sys.stderr)
    print(f"Missing in Landing Page:{len(report.missing_in_landing)} -> {', '.join(report.missing_in_landing)}", file=sys.stderr)
    print(f"Missing in Site Sidebar:{len(report.missing_in_sidebar)} -> {', '.join(report.missing_in_sidebar)}", file=sys.stderr)


if __name__ == "__main__":
    main()
