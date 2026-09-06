#!/usr/bin/env python3
"""
detect_doc_gaps.py - Analyze recent git commits and detect documentation gaps in VitePress / docs.

Usage:
  python detect_doc_gaps.py [--since GIT_REF] [--workspace WORKSPACE_PATH]
"""

import sys
import os
import argparse
import subprocess
import json
import re

if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
if hasattr(sys.stderr, 'reconfigure'):
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

def run_git_command(args, cwd):
    try:
        res = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True, check=True, encoding='utf-8', errors='replace')
        return res.stdout.strip()
    except Exception:
        return None

def get_git_diff_files(since_ref, cwd):
    cmd = ["diff", "--name-status", since_ref, "HEAD"]
    out = run_git_command(cmd, cwd)
    if out is None:
        out = run_git_command(["diff", "--name-status", "HEAD~1", "HEAD"], cwd) or ""
    
    status_map = {}
    for line in out.splitlines():
        parts = line.strip().split(maxsplit=1)
        if len(parts) == 2:
            status, filepath = parts[0], parts[1]
            status_map[filepath] = status
    return status_map

def find_unindexed_doc_files(workspace_path):
    docs_user_dir = os.path.join(workspace_path, "docs", "user")
    config_ts_path = os.path.join(workspace_path, "website", ".vitepress", "config.ts")
    
    if not os.path.exists(docs_user_dir) or not os.path.exists(config_ts_path):
        return []
    
    with open(config_ts_path, "r", encoding="utf-8") as f:
        config_content = f.read()
    
    links = set(re.findall(r"link:\s*['\"]([^'\"]+)['\"]", config_content))
    
    all_doc_files = []
    for root, _, files in os.walk(docs_user_dir):
        for file in files:
            if file.endswith(".md") and file != "README.md":
                rel_path = os.path.relpath(os.path.join(root, file), docs_user_dir).replace("\\", "/")
                route = "/" + rel_path.replace(".md", "")
                if route.endswith("/index"):
                    route = route[:-5]
                all_doc_files.append((rel_path, route))
    
    unindexed = []
    for rel_path, route in all_doc_files:
        if route in ("/", "/en/"):
            continue
        if route not in links:
            unindexed.append((rel_path, route))
    
    return unindexed

def categorize_changes(changed_files):
    categories = {
        "mcp_tools": [],
        "rust_backend": [],
        "frontend_ui": [],
        "skills": [],
        "docs": [],
        "other": []
    }
    
    for filepath, status in changed_files.items():
        if "src-tauri/src/mcp/" in filepath or "mcp" in filepath.lower():
            categories["mcp_tools"].append((filepath, status))
        elif filepath.startswith("src-tauri/"):
            categories["rust_backend"].append((filepath, status))
        elif filepath.startswith("src/"):
            categories["frontend_ui"].append((filepath, status))
        elif ".agents/skills/" in filepath or ".libragent/skills/" in filepath:
            categories["skills"].append((filepath, status))
        elif filepath.startswith("docs/") or filepath.startswith("website/"):
            categories["docs"].append((filepath, status))
        else:
            categories["other"].append((filepath, status))
            
    return categories

def main():
    parser = argparse.ArgumentParser(description="Detect documentation gaps based on git changes and VitePress config")
    parser.add_argument("--since", default="HEAD~5", help="Git reference to compare against HEAD (default: HEAD~5)")
    parser.add_argument("--workspace", default=os.getcwd(), help="Workspace root directory")
    
    args = parser.parse_args()
    workspace = os.path.abspath(args.workspace)
    
    print(f"[SEARCH] Analyzing git changes since {args.since} in {workspace}...")
    changed_files = get_git_diff_files(args.since, workspace)
    categories = categorize_changes(changed_files)
    unindexed_docs = find_unindexed_doc_files(workspace)
    
    report = {
        "git_since": args.since,
        "total_changed_files": len(changed_files),
        "categorized_changes": {
            cat: len(files) for cat, files in categories.items()
        },
        "details": categories,
        "unindexed_vitepress_docs": unindexed_docs
    }
    
    print("\n[SUMMARY] Git Change Summary:")
    for cat, files in categories.items():
        if files:
            print(f"  - {cat}: {len(files)} files")
            
    if unindexed_docs:
        print(f"\n[WARN] Found {len(unindexed_docs)} markdown files in docs/user/ not in VitePress sidebar:")
        for rel_path, route in unindexed_docs:
            print(f"  - {rel_path} (route: {route})")
    else:
        print("\n[OK] All markdown files in docs/user/ are indexed in VitePress sidebar config!")
        
    output_path = os.path.join(workspace, ".agents", "skills", "vitepress-doc-sync", "doc_gaps_report.json")
    try:
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
        print(f"\n[SAVE] Report saved to: {output_path}")
    except Exception as e:
        print(f"[WARN] Could not save report: {e}")

if __name__ == "__main__":
    main()
