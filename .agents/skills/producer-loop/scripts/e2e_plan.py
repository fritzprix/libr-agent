#!/usr/bin/env python3
"""e2e-plan helper: collect merged product PRs and render one auto:e2e checklist issue.

Subcommands:
  collect                        -> JSON: product PRs merged after the cursor, or a noop
  render --collected C --items I -> print the issue body; --out writes findings for file_issue.py
  advance --to ISO               -> move state["e2ePlanCursor"] (run only after the issue is created)

The agent writes the checklist items (I) from the collected PR bodies and linked issues.
This script never creates issues itself; file_issue.py does that (dry run first, then --create).
"""
from __future__ import annotations

import argparse
import fnmatch
import json
import re
import sys
from datetime import datetime
from pathlib import Path

import file_issue
import producer_gate

BASE_BRANCH = "dev/0.9.x"
E2E_LABEL = "auto:e2e"
CURSOR_KEY = "e2ePlanCursor"
DEFAULT_CURSOR = "2026-11-04T07:13:48Z"  # #2561 merge time; PRs up to it are in #2565
MARKER_PREFIX = "producer-loop:e2e-plan="

# A PR whose changed files all match these patterns has no product change.
NON_PRODUCT_PATTERNS = (
    ".agents/*",
    "benchmarks/*",
    "scripts/run-harbor-bench*",
    "docs/*",
    "tests/*",
    "*/tests/*",
    "*.test.ts",
    "*.test.tsx",
    "*.spec.ts",
    "*.spec.tsx",
    "test_*.py",
    "*/test_*.py",
    "*_test.py",
)

SOURCES = ("issue AC", "PR body")
_TAG_RE = re.compile(r"^(auto|manual|blocked: #\d+)$")
_PREFIX_RE = re.compile(r"^[A-Z]{1,3}$")
_LINK_RE = re.compile(r"(?i)\b(?:fix(?:es|ed)?|close[sd]?|resolve[sd]?|refs?|part(?:\s+\d+)?\s+of)\s+#(\d+)")
_HEADING_RE = re.compile(r"^(#{2,3})\s+(.*)$")
_MAX_TEXT = 6000


def gh(*args: str) -> str:
    return file_issue.gh(*args)


def parse_time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def is_product_pr(paths: list[str]) -> bool:
    return any(not any(fnmatch.fnmatch(p, pat) for pat in NON_PRODUCT_PATTERNS) for p in paths)


def scope_prs(body: str) -> set[int]:
    """PR numbers listed in the Scope section. A "Pending ..." line lists PRs that are not covered yet."""
    numbers: set[int] = set()
    in_scope = False
    for line in body.splitlines():
        heading = _HEADING_RE.match(line)
        if heading and len(heading.group(1)) == 2:
            in_scope = heading.group(2).strip().lower() == "scope"
            continue
        if in_scope and not line.lstrip("- ").lower().startswith("pending"):
            numbers.update(int(n) for n in re.findall(r"#(\d+)", line))
    return numbers


def range_marker(first: int, last: int) -> str:
    return f"<!-- {MARKER_PREFIX}{first}-{last} -->"


def linked_issue_numbers(body: str) -> list[int]:
    return sorted({int(n) for n in _LINK_RE.findall(body or "")})


def requirement_sections(body: str) -> str:
    """Requirements and acceptance criteria sections of an issue body, or its start when it has none."""
    kept: list[str] = []
    keep = False
    for line in (body or "").splitlines():
        heading = _HEADING_RE.match(line)
        if heading:
            title = heading.group(2).lower()
            keep = "requirement" in title or "acceptance" in title
        if keep:
            kept.append(line)
    text = "\n".join(kept).strip() or (body or "").strip()
    return text[:_MAX_TEXT]


def open_e2e_issues() -> list[dict]:
    out = gh("issue", "list", "--label", E2E_LABEL, "--state", "open", "--json", "number,body", "--limit", "50")
    return json.loads(out or "[]")


def merged_prs(cursor: str) -> list[dict]:
    out = gh("pr", "list", "--base", BASE_BRANCH, "--state", "merged", "--search", f"merged:>{cursor}",
             "--json", "number,title,mergedAt,body,files", "--limit", "100")
    after = parse_time(cursor)
    prs = [pr for pr in json.loads(out or "[]") if parse_time(pr["mergedAt"]) > after]
    return sorted(prs, key=lambda pr: pr["mergedAt"])


def linked_issue(number: int) -> dict:
    data = json.loads(gh("issue", "view", str(number), "--json", "number,title,body"))
    return {"number": number, "title": data.get("title", ""), "requirements": requirement_sections(data.get("body", ""))}


def collect(state: dict) -> dict:
    cursor = state.get(CURSOR_KEY) or DEFAULT_CURSOR
    issues = open_e2e_issues()
    covered: dict[int, int] = {}
    for issue in issues:
        for number in scope_prs(issue.get("body") or ""):
            covered.setdefault(number, issue["number"])

    product: list[dict] = []
    excluded: list[dict] = []
    for pr in merged_prs(cursor):
        paths = [f["path"] for f in pr.get("files") or []]
        if pr["number"] in covered:
            excluded.append({"number": pr["number"], "reason": f"already in the scope of #{covered[pr['number']]}"})
        elif not is_product_pr(paths):
            excluded.append({"number": pr["number"], "reason": "no product change (skills, bench, docs, or tests only)"})
        else:
            product.append(pr)

    if not product:
        return {"action": "noop", "reason": "no product PRs", "cursor": cursor, "excluded": excluded}

    first, last = product[0]["number"], product[-1]["number"]
    for issue in issues:
        if range_marker(first, last) in (issue.get("body") or ""):
            return {"action": "noop", "reason": f"#{issue['number']} already plans {first}-{last}",
                    "cursor": cursor, "excluded": excluded}

    return {
        "action": "plan",
        "cursor": cursor,
        "cursorAfter": product[-1]["mergedAt"],
        "range": [first, last],
        "excluded": excluded,
        "prs": [{
            "number": pr["number"],
            "title": pr["title"],
            "mergedAt": pr["mergedAt"],
            "body": (pr.get("body") or "")[:_MAX_TEXT],
            "files": [f["path"] for f in pr.get("files") or []],
            "linkedIssues": [linked_issue(n) for n in linked_issue_numbers(pr.get("body") or "")],
        } for pr in product],
    }


def _validate_items(items: dict) -> None:
    if not isinstance(items, dict):
        raise ValueError("items must be a JSON object")
    top = items.get("top10")
    if not (isinstance(top, list) and 1 <= len(top) <= 10 and all(isinstance(t, str) and t for t in top)):
        raise ValueError("top10 must be a list of 1 to 10 strings")
    areas = items.get("areas")
    if not (isinstance(areas, list) and areas):
        raise ValueError("areas must be a non-empty list")
    prefixes: set[str] = set()
    for area in areas:
        if not isinstance(area, dict):
            raise ValueError("each area must be a JSON object")
        prefix, title, rows = area.get("prefix"), area.get("title"), area.get("items")
        if not (isinstance(prefix, str) and _PREFIX_RE.match(prefix)) or prefix in prefixes:
            raise ValueError(f"area prefix must be 1-3 unique capital letters: {prefix!r}")
        prefixes.add(prefix)
        if not (isinstance(title, str) and title and isinstance(rows, list) and rows):
            raise ValueError(f"area {prefix} needs a title and items")
        for row in rows:
            if not isinstance(row, dict):
                raise ValueError(f"area {prefix}: each item must be a JSON object")
            tag, text, prs, source = row.get("tag"), row.get("text"), row.get("prs"), row.get("source")
            if not (isinstance(tag, str) and _TAG_RE.match(tag)):
                raise ValueError(f"area {prefix}: tag must be auto, manual, or 'blocked: #N': {tag!r}")
            if not (isinstance(text, str) and text.strip()):
                raise ValueError(f"area {prefix}: item text is required")
            if not (isinstance(prs, list) and prs and all(isinstance(n, int) and not isinstance(n, bool) for n in prs)):
                raise ValueError(f"area {prefix}: prs must be a non-empty list of PR numbers")
            if source not in SOURCES:
                raise ValueError(f"area {prefix}: source must be one of {SOURCES}: {source!r}")


def render_body(collected: dict, items: dict) -> str:
    """Issue body in the #2565 format. Raises ValueError on bad items or private text."""
    if collected.get("action") != "plan":
        raise ValueError("collected data has no PRs to plan")
    _validate_items(items)
    prs = collected["prs"]
    first, last = collected["range"]
    merged = ", ".join(f"#{pr['number']}" for pr in prs)
    lines = [
        "## Summary",
        f"Verify the product PRs merged into {BASE_BRANCH} from #{first} to #{last} "
        f"({prs[0]['mergedAt'][:10]} to {prs[-1]['mergedAt'][:10]}) on a built app. Record a result for each item.",
        "The e2e task runs the `[auto]` items through libr-tool-e2e-harness-loop. A person runs the `[manual]` items.",
        "",
        "## Scope",
        f"- Merged: {merged}.",
    ]
    for ex in collected.get("excluded") or []:
        lines.append(f"- Out of scope: #{ex['number']} ({ex['reason']}).")
    lines += ["", "## Risk-first top 10"]
    lines += [f"{i}. {text}" for i, text in enumerate(items["top10"], 1)]
    lines += [
        "",
        "## Checklist",
        "Format: **ID** [auto|manual|blocked: #N] – precondition → steps → expected result (PR, source: issue AC | PR body).",
    ]
    for index, area in enumerate(items["areas"], 1):
        lines += ["", f"### {index}. {area['title']}"]
        for number, row in enumerate(area["items"], 1):
            refs = ", ".join(f"#{n}" for n in row["prs"])
            lines.append(f"- **{area['prefix']}-{number}** [{row['tag']}] – {row['text'].strip()} "
                         f"({refs}, source: {row['source']})")
    lines += [
        "",
        "## Acceptance criteria",
        "- Each item has a recorded result: `pass`, `fail`, or `blocked`.",
        "- Each `[auto]` item has a row in the e2e-harness results comment.",
        "- Each `pass` or `fail` has evidence: a session id, a log excerpt, or a screenshot. The evidence contains no secret value.",
        "- Each `fail` has a linked bug issue.",
        "- Each `blocked` item names its blocker issue or PR.",
        "",
        "## Notes",
        "- Use dummy secrets only, for example `dummy-token-abcd`. Do not enter real credentials.",
        "- Use `unsafe` mode only in a scratch directory. Use the Master Mind assistant.",
        "- sudo, OS keychain, dialog answers, and visual checks are `[manual]`.",
        "- Source: PR bodies and linked issue bodies. The expected text was not checked against the code.",
        "- Build: follow .agents/skills/libr-e2e-verify/references/e2e-build-precondition.md. Record buildSha in the results comment.",
    ]
    lines += [f"- {note}" for note in items.get("notes") or [] if isinstance(note, str) and note]
    lines += ["", range_marker(first, last)]
    body = "\n".join(lines)
    reason = file_issue.private_text_reason(body)
    if reason:
        raise ValueError(f"issue body refused: {reason}")
    return body


def render_findings(collected: dict, items: dict) -> list[dict]:
    first, last = collected["range"]
    body = render_body(collected, items)
    return [{"key": f"e2e-plan:{first}-{last}",
             "title": f"E2E verification: PRs merged into {BASE_BRANCH} #{first}..#{last}",
             "body": body}]


def advance(state: dict, to: str) -> bool:
    """Move the cursor forward to `to`. Returns False when `to` is not newer."""
    current = state.get(CURSOR_KEY) or DEFAULT_CURSOR
    if parse_time(to) <= parse_time(current):
        return False
    state[CURSOR_KEY] = to
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter,
                                     allow_abbrev=False)
    parser.add_argument("--state-file", type=Path, default=None)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("collect")
    render = sub.add_parser("render")
    render.add_argument("--collected", required=True, type=Path)
    render.add_argument("--items", required=True, type=Path)
    render.add_argument("--out", type=Path, default=None, help="Write findings JSON for file_issue.py")
    adv = sub.add_parser("advance")
    adv.add_argument("--to", required=True)
    args = parser.parse_args(argv)

    state_path = args.state_file or producer_gate.default_state_path()
    try:
        if args.command == "collect":
            state = producer_gate.tick_gate.load_state(state_path)
            print(json.dumps(collect(state), ensure_ascii=False, indent=2))
        elif args.command == "render":
            collected = json.loads(args.collected.read_text(encoding="utf-8"))
            items = json.loads(args.items.read_text(encoding="utf-8"))
            findings = render_findings(collected, items)
            if args.out:
                args.out.write_text(json.dumps(findings, ensure_ascii=False, indent=2), encoding="utf-8")
            print(findings[0]["body"])
        else:
            parse_time(args.to)
            state = producer_gate.tick_gate.load_state(state_path)
            if advance(state, args.to):
                producer_gate.tick_gate.save_state(state_path, state)
                print(f"CURSOR {args.to}")
            else:
                print(f"CURSOR unchanged {state.get(CURSOR_KEY) or DEFAULT_CURSOR}")
    except (file_issue.GhError, ValueError, OSError, KeyError) as e:
        sys.stderr.write(f"e2e_plan error: {e}\n")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
