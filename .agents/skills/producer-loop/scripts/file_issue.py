#!/usr/bin/env python3
"""File producer-loop findings as GitHub issues, with dedup and a per-run cap.

Input is a JSON list of findings: [{"key": "...", "title": "...", "body": "..."}].
`key` is a stable identifier of the problem (for example "path:rule"), not prose.

Default is a dry run. Issues are created only with --create.
Producers only create issues: this script never edits, comments on, labels, or closes
existing items.

Cap: at most MAX_NEW_ISSUES_PER_RUN new issues per invocation (one invocation = one
run). Override per task in state.json: {"maxNewIssues": {"security": 5}}.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

import producer_gate

MAX_NEW_ISSUES_PER_RUN = 3
PRODUCER_LABEL = "auto:producer"
MARKER_PREFIX = "producer-loop:fingerprint="

# Issue bodies are public. Refuse text that looks like a secret or a private path.
_PRIVATE_PATTERNS = (
    re.compile(r"\b(ghp|gho|ghs|github_pat)_[A-Za-z0-9_]{16,}"),
    re.compile(r"\bsk-[A-Za-z0-9_-]{16,}"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"(?i)\b(api[_-]?key|token|secret|password)\s*[:=]\s*\S{8,}"),
    re.compile(r"/home/[^/\s]+/|/Users/[^/\s]+/|[A-Za-z]:\\Users\\"),
)


class GhError(RuntimeError):
    pass


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args[:3])} failed: {result.stderr.strip()}")
    return result.stdout


def fingerprint(task: str, key: str) -> str:
    return hashlib.sha256(f"{task}\0{key}".encode("utf-8")).hexdigest()[:16]


def marker(fp: str) -> str:
    return f"<!-- {MARKER_PREFIX}{fp} -->"


def private_text_reason(text: str) -> str | None:
    for pattern in _PRIVATE_PATTERNS:
        if pattern.search(text):
            return f"matches private-text pattern {pattern.pattern!r}"
    return None


def issue_exists(fp: str) -> int | None:
    """Number of an open or closed issue whose body holds the marker, if any."""
    out = gh("issue", "list", "--state", "all", "--search", f"{fp} in:body",
             "--json", "number,body", "--limit", "20")
    for item in json.loads(out or "[]"):
        if marker(fp) in (item.get("body") or ""):
            return item["number"]
    return None


def cap_for(state: dict, task: str) -> int:
    value = (state.get("maxNewIssues") or {}).get(task)
    if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
        return value
    return MAX_NEW_ISSUES_PER_RUN


def ensure_labels(labels: list[str]) -> None:
    existing = {item["name"] for item in json.loads(gh("label", "list", "--limit", "200", "--json", "name") or "[]")}
    for label in labels:
        if label not in existing:
            gh("label", "create", label, "--description", "Created by producer-loop")


def file_findings(task: str, findings: list[dict], state: dict, create: bool) -> list[dict]:
    """Return one result per finding: skipped (with reason), would-create, or created."""
    cap = cap_for(state, task)
    labels = [PRODUCER_LABEL, f"auto:{task}"]
    known = state.setdefault("fingerprints", {})
    results: list[dict] = []
    created = 0
    labels_ready = False
    for finding in findings:
        key, title, body = finding.get("key"), finding.get("title"), finding.get("body")
        if not (isinstance(key, str) and key and isinstance(title, str) and title and isinstance(body, str) and body):
            results.append({"key": key, "action": "skipped", "reason": "key, title and body are required"})
            continue
        fp = fingerprint(task, key)
        if fp in known:
            results.append({"key": key, "fingerprint": fp, "action": "skipped", "reason": f"known in state (#{known[fp]})"})
            continue
        reason = private_text_reason(f"{title}\n{body}")
        if reason:
            results.append({"key": key, "fingerprint": fp, "action": "skipped", "reason": reason})
            continue
        existing = issue_exists(fp)
        if existing is not None:
            known[fp] = existing
            results.append({"key": key, "fingerprint": fp, "action": "skipped", "reason": f"issue #{existing} has the marker"})
            continue
        if created >= cap:
            results.append({"key": key, "fingerprint": fp, "action": "skipped", "reason": f"cap of {cap} new issues per run"})
            continue
        created += 1
        if not create:
            results.append({"key": key, "fingerprint": fp, "action": "would-create", "labels": labels})
            continue
        if not labels_ready:
            ensure_labels(labels)
            labels_ready = True
        url = gh("issue", "create", "--title", title, "--body", f"{body}\n\n{marker(fp)}\n",
                 *[arg for label in labels for arg in ("--label", label)]).strip()
        number = int(url.rsplit("/", 1)[-1])
        known[fp] = number
        results.append({"key": key, "fingerprint": fp, "action": "created", "issue": number, "url": url})
    return results


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--task", required=True, choices=producer_gate.TASKS)
    parser.add_argument("--findings", required=True, type=Path, help="JSON list of findings")
    parser.add_argument("--create", action="store_true", help="Create issues (default is a dry run)")
    parser.add_argument("--state-file", type=Path, default=None)
    args = parser.parse_args(argv)

    state_path = args.state_file or producer_gate.default_state_path()
    state = producer_gate.tick_gate.load_state(state_path)
    try:
        findings = json.loads(args.findings.read_text(encoding="utf-8"))
        if not isinstance(findings, list):
            raise ValueError("findings file must hold a JSON list")
        results = file_findings(args.task, findings, state, args.create)
    except (GhError, ValueError, OSError) as e:
        sys.stderr.write(f"file_issue error: {e}\n")
        return 3
    if args.create:
        producer_gate.tick_gate.save_state(state_path, state)
    print(json.dumps({"dryRun": not args.create, "results": results}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
