#!/usr/bin/env python3
"""Issue checklist mode helper: parse an auto:e2e issue and render its one results comment.

Subcommands:
  parse --issue N                          -> JSON: [auto] items to run and untouched items
  comment --issue N [--results R] [--post] -> print the results comment; --post creates or updates it

Results file R: [{"id": "E-1", "verdict": "pass|fail|blocked", "session": "...",
"excerpt": "...", "artifact": "...", "time": "2026-11-04T09:00Z"}].
A result that holds a dummy secret becomes `fail`, and the value is replaced with ***.
Default is a dry run. The script never edits the issue body, labels, or state.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

MARKER_PREFIX = "e2e-harness:results issue="
VERDICTS = ("pass", "fail", "blocked")
UNTAGGED_NOTE = "untagged → no auto items"
_ITEM_RE = re.compile(r"^\s*-\s+\*\*([A-Z]{1,3}-\d+)[^*]*\*\*\s*(?:\[([^\]]+)\])?\s*[–-]\s*(.*)$")
_TAG_RE = re.compile(r"^(auto|manual|blocked: #\d+)$")
_DUMMY_RE = re.compile(r"`(dummy[^`]*)`")
_COMMENT_ID_RE = re.compile(r"#issuecomment-(\d+)$")
_MAX_EXCERPT = 200


class GhError(RuntimeError):
    pass


def gh(*args: str, stdin: str | None = None) -> str:
    result = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args[:3])} failed: {result.stderr.strip()}")
    return result.stdout


def parse_items(body: str) -> list[dict]:
    """Checklist items. `tag` is None when the item has no valid [auto|manual|blocked: #N] tag."""
    items = []
    for line in body.splitlines():
        match = _ITEM_RE.match(line)
        if match:
            tag = (match.group(2) or "").strip()
            items.append({"id": match.group(1), "tag": tag if _TAG_RE.match(tag) else None,
                          "text": match.group(3).strip()})
    return items


def split_items(items: list[dict]) -> tuple[list[dict], list[dict]]:
    auto = [item for item in items if item["tag"] == "auto"]
    untouched = [item for item in items if item["tag"] != "auto"]
    return auto, untouched


def dummy_secrets(body: str, extra: list[str] | None = None) -> list[str]:
    """Dummy secret values named in the issue (backticked `dummy...`) plus extras, longest first."""
    values = {v for v in _DUMMY_RE.findall(body) if v.strip()} | {v for v in extra or [] if v.strip()}
    return sorted(values, key=len, reverse=True)


def scrub(text: str, secrets: list[str]) -> tuple[str, bool]:
    leaked = False
    for secret in secrets:
        if secret in text:
            leaked = True
            text = text.replace(secret, "***")
    return text, leaked


def apply_results(results: list[dict], secrets: list[str]) -> list[dict]:
    rows = []
    for result in results:
        if not isinstance(result, dict) or not isinstance(result.get("id"), str):
            raise ValueError("each result needs an id")
        verdict = result.get("verdict")
        if verdict not in VERDICTS:
            raise ValueError(f"{result['id']}: verdict must be one of {VERDICTS}")
        evidence = {}
        leaked = False
        for key in ("session", "excerpt", "artifact"):
            value = str(result.get(key) or "")
            evidence[key], hit = scrub(value, secrets)
            leaked = leaked or hit
        if leaked:
            verdict = "fail"
            evidence["excerpt"] = f"dummy secret in result; {evidence['excerpt']}"
        rows.append({"id": result["id"], "verdict": verdict, **evidence,
                     "time": str(result.get("time") or datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%MZ"))})
    return rows


def results_marker(issue: int) -> str:
    return f"<!-- {MARKER_PREFIX}{issue} -->"


def _cell(text: str) -> str:
    return " ".join(text.split()).replace("|", "\\|")


def render_comment(
    issue: int,
    items: list[dict],
    rows: list[dict],
    build_sha: str | None = None,
    build_dirty: bool | None = None,
) -> str:
    auto, untouched = split_items(items)
    lines = [results_marker(issue), f"## E2E harness results (#{issue})", "",
             "libr-tool-e2e-harness-loop, issue checklist mode. Only `[auto]` items run.", ""]
    if build_sha:
        dirty = "yes" if build_dirty else "no"
        lines += [f"Build: `{build_sha}` (dirty: {dirty})", ""]
    if not items:
        lines.append("No checklist items found.")
    elif not auto and all(item["tag"] is None for item in items):
        lines.append(UNTAGGED_NOTE)
    if rows:
        lines += ["", "| ID | 결과 | 증거 | 실행 시각(UTC) |", "| --- | --- | --- | --- |"]
        for row in rows:
            parts = [f"session `{row['session']}`" if row["session"] else "",
                     f"\"{row['excerpt'][:_MAX_EXCERPT]}\"" if row["excerpt"] else "",
                     f"`{row['artifact']}`" if row["artifact"] else ""]
            evidence = "; ".join(p for p in parts if p) or "-"
            lines.append(f"| {row['id']} | {row['verdict']} | {_cell(evidence)} | {row['time']} |")
    elif auto:
        lines.append(f"{len(auto)} `[auto]` items found. No results yet.")
    if untouched:
        groups: dict[str, list[str]] = {}
        for item in untouched:
            label = "untagged" if item["tag"] is None else item["tag"].split(":")[0]
            groups.setdefault(label, []).append(item["id"])
        summary = "; ".join(f"{label}: {', '.join(ids)}" for label, ids in groups.items())
        lines += ["", f"Untouched ({len(untouched)}): {summary}."]
    return "\n".join(lines) + "\n"


def comment_action(comments: list[dict], issue: int) -> tuple[str, str | None]:
    """("update", comment id) when a comment holds the marker, else ("create", None)."""
    for comment in comments:
        if results_marker(issue) in (comment.get("body") or ""):
            match = _COMMENT_ID_RE.search(comment.get("url") or "")
            if match:
                return "update", match.group(1)
    return "create", None


def fetch_issue(issue: int) -> dict:
    return json.loads(gh("issue", "view", str(issue), "--json", "body,comments"))


def post_comment(issue: int, action: str, comment_id: str | None, text: str) -> None:
    if action == "update" and comment_id:
        gh("api", "-X", "PATCH", f"repos/{{owner}}/{{repo}}/issues/comments/{comment_id}", "--input", "-",
           stdin=json.dumps({"body": text}))
    else:
        gh("issue", "comment", str(issue), "--body-file", "-", stdin=text)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter,
                                     allow_abbrev=False)
    sub = parser.add_subparsers(dest="command", required=True)
    parse_cmd = sub.add_parser("parse")
    parse_cmd.add_argument("--issue", required=True, type=int)
    comment_cmd = sub.add_parser("comment")
    comment_cmd.add_argument("--issue", required=True, type=int)
    comment_cmd.add_argument("--results", type=Path, default=None)
    comment_cmd.add_argument("--secret", action="append", default=[], help="Extra dummy value to scrub")
    comment_cmd.add_argument("--build-sha", default=None, help="Recorded buildSha from the running app")
    comment_cmd.add_argument("--build-dirty", action="store_true", help="Set when buildDirty was true")
    comment_cmd.add_argument("--post", action="store_true", help="Create or update the comment (default: dry run)")
    args = parser.parse_args(argv)

    try:
        data = fetch_issue(args.issue)
        body = data.get("body") or ""
        items = parse_items(body)
        if args.command == "parse":
            auto, untouched = split_items(items)
            print(json.dumps({"issue": args.issue, "auto": auto, "untouched": [i["id"] for i in untouched],
                              "untagged": bool(items) and all(i["tag"] is None for i in items)},
                             ensure_ascii=False, indent=2))
            return 0
        results = json.loads(args.results.read_text(encoding="utf-8")) if args.results else []
        if not isinstance(results, list):
            raise ValueError("results file must hold a JSON list")
        rows = apply_results(results, dummy_secrets(body, args.secret))
        text = render_comment(args.issue, items, rows, build_sha=args.build_sha,
                             build_dirty=args.build_dirty if args.build_sha else None)
        action, comment_id = comment_action(data.get("comments") or [], args.issue)
        print(f"ACTION {action}{' ' + comment_id if comment_id else ''}{'' if args.post else ' (dry run)'}")
        print(text)
        if args.post:
            post_comment(args.issue, action, comment_id, text)
    except (GhError, ValueError, OSError) as e:
        sys.stderr.write(f"checklist error: {e}\n")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
