#!/usr/bin/env python3
"""Validate producer-loop issues for gatekeeper-loop.

Usage:
  issue_validation.py next
      -> JSON of the oldest open `auto:producer` issue without `validated` or `rejected`,
         or `NONE` (exit 1).
  issue_validation.py judge N --verdict validated|rejected --reason-file F
      -> add the verdict label, append the reason and evidence to the issue body, and
         count the verdict per producer task in gatekeeper-loop state.

The gatekeeper only judges validity. This script never adds `approved`, closes issues,
comments, or changes any label other than `validated` / `rejected`.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import review_queue

PRODUCER_LABEL = "auto:producer"
VERDICTS = ("validated", "rejected")
TASK_LABEL_PREFIX = "auto:"
SECTION_MARKER = "<!-- gatekeeper-loop: issue-validation verdict={verdict} -->"


class GhError(RuntimeError):
    pass


def gh(*args: str, stdin: str | None = None) -> str:
    result = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args[:3])} failed: {result.stderr.strip()}")
    return result.stdout


def label_names(issue: dict) -> list[str]:
    return [label.get("name", "") for label in issue.get("labels") or []]


def producer_task(labels: list[str]) -> str:
    tasks = [name[len(TASK_LABEL_PREFIX):] for name in labels
             if name.startswith(TASK_LABEL_PREFIX) and name != PRODUCER_LABEL]
    return tasks[0] if tasks else "unknown"


def is_unjudged_producer_issue(labels: list[str]) -> bool:
    return PRODUCER_LABEL in labels and not any(verdict in labels for verdict in VERDICTS)


def next_issue() -> dict | None:
    out = gh("issue", "list", "--state", "open", "--label", PRODUCER_LABEL,
             "--json", "number,title,labels,createdAt", "--limit", "100")
    candidates = [issue for issue in json.loads(out or "[]")
                  if is_unjudged_producer_issue(label_names(issue))]
    if not candidates:
        return None
    oldest = min(candidates, key=lambda issue: (issue.get("createdAt") or "", issue["number"]))
    labels = label_names(oldest)
    return {"number": oldest["number"], "title": oldest.get("title", ""),
            "task": producer_task(labels), "labels": labels}


def judge(number: int, verdict: str, reason: str, state: dict) -> dict:
    if verdict not in VERDICTS:
        raise ValueError(f"verdict must be one of {VERDICTS}")
    if not reason.strip():
        raise ValueError("reason and evidence are required")
    issue = json.loads(gh("issue", "view", str(number), "--json", "number,state,labels,body") or "{}")
    labels = label_names(issue)
    if issue.get("state") != "OPEN":
        raise ValueError(f"issue #{number} is not open")
    if not is_unjudged_producer_issue(labels):
        raise ValueError(f"issue #{number} is not an unjudged {PRODUCER_LABEL} issue")

    body = (issue.get("body") or "").rstrip()
    section = (f"{SECTION_MARKER.format(verdict=verdict)}\n"
               f"## Gatekeeper validation: {verdict}\n\n{reason.strip()}\n")
    # --body-file - keeps long bodies under the Windows command-line limit.
    gh("issue", "edit", str(number), "--body-file", "-", stdin=f"{body}\n\n{section}")
    existing = {item["name"] for item in json.loads(gh("label", "list", "--limit", "200", "--json", "name") or "[]")}
    if verdict not in existing:
        gh("label", "create", verdict, "--description", "Set by gatekeeper-loop issue validation")
    gh("issue", "edit", str(number), "--add-label", verdict)

    task = producer_task(labels)
    counts = state.setdefault("issueValidation", {}).setdefault(task, {v: 0 for v in VERDICTS})
    counts[verdict] = counts.get(verdict, 0) + 1
    return {"issue": number, "task": task, "verdict": verdict, "counts": counts}


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--state-file", type=Path, default=None)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("next")
    judge_parser = sub.add_parser("judge")
    judge_parser.add_argument("number", type=int)
    judge_parser.add_argument("--verdict", required=True, choices=VERDICTS)
    judge_parser.add_argument("--reason-file", required=True, type=Path)
    args = parser.parse_args(argv)

    try:
        if args.command == "next":
            issue = next_issue()
            if issue is None:
                print("NONE")
                return 1
            print(json.dumps(issue, ensure_ascii=False, indent=2))
            return 0

        state_path = args.state_file or review_queue.get_default_state_file()
        state = review_queue.load_state(state_path)
        reason = args.reason_file.read_text(encoding="utf-8")
        result = judge(args.number, args.verdict, reason, state)
        save_state(state_path, state)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (GhError, ValueError, OSError) as e:
        sys.stderr.write(f"issue_validation error: {e}\n")
        return 3


if __name__ == "__main__":
    sys.exit(main())
