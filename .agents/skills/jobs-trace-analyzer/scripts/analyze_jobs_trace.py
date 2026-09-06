#!/usr/bin/env python3
"""Analyze LibrAgent jobs/ trial results and ATIF trajectories.

Relationship to harbor-harness-improvement-loop:
  This script is a lighter inventory/categorization pass over jobs/<run>/.
  For evidence-backed BM→fix cycles, prefer
  `.agents/skills/harbor-harness-improvement-loop/` (reward.txt SSOT, ATIF-first
  metrics, explicit heuristic ≠ verified failure).

Observation "error" detection is a keyword heuristic and must not be treated as
a verified tool failure.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional


ERROR_SIGNAL = re.compile(
    r"\b(error|failed|failure|invalid|not found|permission denied|"
    r"timed? out|timeout|traceback|exception)\b",
    re.IGNORECASE,
)

NETWORK_API_PATTERN = re.compile(
    r"(Connection(?:Error|Refused|Reset)|RateLimit(?:Error)?|"
    r"API(?:Connection|Status|Response|Timeout|Error)|"
    r"\b(?:ECONNREFUSED|ETIMEDOUT|ENOTFOUND|SocketTimeout|ConnectTimeout)\b)",
    re.IGNORECASE,
)

FAILURE_CATEGORY_ORDER = [
    "TIMEOUT_EXCEEDED",
    "NETWORK_API_ERROR",
    "TOOL_EXECUTION_ERROR",
    "INITIALIZATION_OR_EARLY_ABORT",
    "AGENT_LOOP_OR_STUCK",
    "HIGH_TURN_COUNT",
    "VERIFIER_FAILED_WRONG_STATE",
]


def category_sort_key(category: str) -> int:
    try:
        return FAILURE_CATEGORY_ORDER.index(category)
    except ValueError:
        return 999


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Analyze LibrAgent jobs trace and harness bottlenecks."
    )
    parser.add_argument(
        "job_path",
        nargs="?",
        default=None,
        help="Path to job directory (e.g., jobs/2026-08-09__14-44-56)",
    )
    parser.add_argument(
        "--jobs-dir", default="jobs", help="Root jobs directory (default: jobs)"
    )
    parser.add_argument("--output", help="Output JSON/Markdown report path")
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print per-task tool lists and heuristic observation hits",
    )
    return parser.parse_args()


def find_latest_job(jobs_dir: Path) -> Optional[Path]:
    if not jobs_dir.exists():
        return None
    job_dirs = [d for d in jobs_dir.iterdir() if d.is_dir() and (d / "result.json").exists()]
    if not job_dirs:
        return None
    job_dirs.sort(key=lambda p: p.name, reverse=True)
    return job_dirs[0]


def load_json(path: Path) -> Optional[Dict[str, Any]]:
    try:
        if not path.exists():
            return None
        data = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(data, dict):
            return data
    except Exception as e:
        sys.stderr.write(f"Warning: Failed to load {path}: {e}\n")
    return None


def number(value: Any) -> Optional[float]:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value.strip())
        except ValueError:
            return None
    return None


def observation_texts(step: dict[str, Any]) -> Iterable[str]:
    observation = step.get("observation")
    if not isinstance(observation, dict):
        return
    results = observation.get("results")
    if not isinstance(results, list):
        return
    for result in results:
        if not isinstance(result, dict):
            continue
        content = result.get("content")
        if isinstance(content, str):
            yield content
        elif content is not None:
            try:
                yield json.dumps(content, ensure_ascii=False)
            except (TypeError, ValueError):
                yield str(content)


def extract_tools_from_metadata_trajectory(
    trajectory: Any,
) -> tuple[List[str], List[str], int]:
    """Parse OpenAI-style chat trajectory in agent_result.metadata.
    
    Extracts tool call names, structured error tags, and repeated adjacent calls.
    """
    tool_calls: List[str] = []
    explicit_errors: List[str] = []
    repeated_adjacent_calls = 0
    previous_call: Optional[str] = None

    if not isinstance(trajectory, list):
        return tool_calls, explicit_errors, repeated_adjacent_calls

    for msg in trajectory:
        if not isinstance(msg, dict):
            continue
        t_calls = msg.get("toolCalls") or msg.get("tool_calls")
        if isinstance(t_calls, list):
            for tc in t_calls:
                if not isinstance(tc, dict):
                    continue
                fn_info = tc.get("function")
                if isinstance(fn_info, dict):
                    fn_name = fn_info.get("name")
                    fn_args = fn_info.get("arguments")
                else:
                    fn_name = tc.get("name")
                    fn_args = tc.get("arguments") or tc.get("args")

                if fn_name:
                    fn_str = str(fn_name)
                    tool_calls.append(fn_str)
                    try:
                        encoded = json.dumps(
                            fn_args, sort_keys=True, ensure_ascii=False
                        )
                    except (TypeError, ValueError):
                        encoded = repr(fn_args)
                    call_key = f"{fn_str}:{encoded}"
                    if call_key == previous_call:
                        repeated_adjacent_calls += 1
                    previous_call = call_key

        # Rare: some exporters put a structured error on tool messages.
        if msg.get("role") == "tool" and msg.get("error"):
            explicit_errors.append(str(msg.get("error")))

    return tool_calls, explicit_errors, repeated_adjacent_calls


def extract_tools_from_atif(
    trajectory_json: Dict[str, Any],
) -> tuple[List[str], int, int, int]:
    """Parse ATIF steps[].tool_calls / observation (Harbor-compatible)."""
    steps = trajectory_json.get("steps")
    if not isinstance(steps, list):
        steps = []

    tool_calls: List[str] = []
    heuristic_error_observations = 0
    repeated_adjacent_calls = 0
    previous_call: Optional[str] = None

    for raw_step in steps:
        if not isinstance(raw_step, dict):
            continue
        calls = raw_step.get("tool_calls")
        if isinstance(calls, list):
            for raw_call in calls:
                if not isinstance(raw_call, dict):
                    continue
                name = raw_call.get("function_name") or raw_call.get("name")
                tool_calls.append(str(name or "<unknown>"))
                try:
                    encoded = json.dumps(
                        raw_call.get("arguments"), sort_keys=True, ensure_ascii=False
                    )
                except (TypeError, ValueError):
                    encoded = repr(raw_call.get("arguments"))
                call_key = f"{name}:{encoded}"
                if call_key == previous_call:
                    repeated_adjacent_calls += 1
                previous_call = call_key
        for text in observation_texts(raw_step):
            if ERROR_SIGNAL.search(text):
                heuristic_error_observations += 1

    return (
        tool_calls,
        len(steps),
        heuristic_error_observations,
        repeated_adjacent_calls,
    )


def resolve_reward(task_dir: Path, result_json: Dict[str, Any]) -> float:
    reward_path = task_dir / "verifier" / "reward.txt"
    if reward_path.exists():
        try:
            raw = reward_path.read_text(encoding="utf-8").strip()
            parsed = number(raw)
            if parsed is not None:
                return parsed
        except (OSError, UnicodeError) as e:
            sys.stderr.write(f"Warning: Failed to read {reward_path}: {e}\n")

    verifier_res = result_json.get("verifier_result")
    if isinstance(verifier_res, dict):
        rewards = verifier_res.get("rewards")
        if isinstance(rewards, dict):
            parsed = number(rewards.get("reward"))
            if parsed is not None:
                return parsed
    return 0.0


def categorize_failure(
    *,
    reward: float,
    exception_name: Optional[str],
    turns_count: int,
    explicit_tool_errors: List[str],
    repeated_adjacent_calls: int,
) -> str:
    if reward >= 1.0:
        return "SUCCESS"
    if exception_name and "Timeout" in exception_name:
        return "TIMEOUT_EXCEEDED"
    if exception_name and NETWORK_API_PATTERN.search(exception_name):
        return "NETWORK_API_ERROR"
    if explicit_tool_errors:
        # Only structured/exporter errors — not observation keyword heuristics.
        return "TOOL_EXECUTION_ERROR"
    if turns_count == 0:
        return "INITIALIZATION_OR_EARLY_ABORT"
    # Detect tight loop/stuckness early without arbitrary high turn count barrier
    if repeated_adjacent_calls >= 3:
        return "AGENT_LOOP_OR_STUCK"
    if turns_count > 30:
        return "HIGH_TURN_COUNT"
    return "VERIFIER_FAILED_WRONG_STATE"


def analyze_task_dir(task_dir: Path) -> Dict[str, Any]:
    task_name = task_dir.name
    result_json = load_json(task_dir / "result.json") or {}
    agent_dir = task_dir / "agent"
    trajectory_json = (
        load_json(agent_dir / "trajectory.json") if agent_dir.exists() else None
    )
    exception_file = task_dir / "exception.txt"
    exception_text = (
        exception_file.read_text(encoding="utf-8") if exception_file.exists() else None
    )

    reward = resolve_reward(task_dir, result_json)

    exception_name = None
    exc_info = result_json.get("exception_info")
    if isinstance(exc_info, dict) and exc_info.get("exception_type"):
        exception_name = str(exc_info.get("exception_type"))
    elif exception_text:
        exception_name = exception_text.strip().split("\n")[0]

    turns_count = 0
    tool_calls: List[str] = []
    explicit_tool_errors: List[str] = []
    heuristic_error_observations = 0
    repeated_adjacent_calls = 0
    tool_source = "none"

    agent_res = result_json.get("agent_result")
    metadata: Dict[str, Any] = {}
    if isinstance(agent_res, dict):
        raw_meta = agent_res.get("metadata")
        if isinstance(raw_meta, dict):
            metadata = raw_meta
            turns_count = int(number(metadata.get("n_turns")) or 0)
            meta_tools, meta_errors, meta_repeats = extract_tools_from_metadata_trajectory(
                metadata.get("trajectory")
            )
            explicit_tool_errors.extend(meta_errors)
            repeated_adjacent_calls = meta_repeats
            declared_count = number(metadata.get("tool_calls_count"))
            if meta_tools:
                tool_calls = meta_tools
                tool_source = "metadata.trajectory"
            elif declared_count is not None and declared_count > 0:
                # Count known but names unavailable — keep placeholders out of freq map.
                tool_source = "metadata.tool_calls_count"
                tool_calls = ["<metadata_count_only>"] * int(declared_count)

    if trajectory_json:
        (
            atif_tools,
            atif_steps,
            atif_heuristics,
            atif_repeats,
        ) = extract_tools_from_atif(trajectory_json)
        if not turns_count:
            turns_count = atif_steps
        heuristic_error_observations = atif_heuristics
        repeated_adjacent_calls = max(repeated_adjacent_calls, atif_repeats)
        # ATIF is SSOT for tool names when metadata trajectory is empty/missing.
        if atif_tools and tool_source in {
            "none",
            "metadata.tool_calls_count",
        }:
            tool_calls = atif_tools
            tool_source = "atif.trajectory"
        elif atif_tools and not tool_calls:
            tool_calls = atif_tools
            tool_source = "atif.trajectory"

    named_tool_calls = [t for t in tool_calls if t != "<metadata_count_only>"]

    failure_category = categorize_failure(
        reward=reward,
        exception_name=exception_name,
        turns_count=turns_count,
        explicit_tool_errors=explicit_tool_errors,
        repeated_adjacent_calls=repeated_adjacent_calls,
    )

    return {
        "task_name": task_name,
        "reward": reward,
        "reward_bool": reward >= 1.0,
        "exception": exception_name,
        "failure_category": failure_category,
        "turns_count": turns_count,
        "tool_calls_count": len(tool_calls),
        "tool_calls": named_tool_calls,
        "tool_source": tool_source,
        "tool_errors_count": len(explicit_tool_errors),
        "explicit_tool_errors": explicit_tool_errors,
        "heuristic_error_observations": heuristic_error_observations,
        "repeated_adjacent_calls": repeated_adjacent_calls,
        "path": str(task_dir),
    }


def main() -> None:
    args = parse_args()
    if args.job_path:
        job_dir = Path(args.job_path)
    else:
        job_dir = find_latest_job(Path(args.jobs_dir))

    if not job_dir or not job_dir.exists():
        sys.stderr.write(
            f"Error: Job directory '{args.job_path or args.jobs_dir}' not found.\n"
        )
        sys.exit(1)

    print(f"=== Analyzing Job Trace: {job_dir.name} ===")

    task_dirs = [
        d for d in job_dir.iterdir() if d.is_dir() and (d / "result.json").exists()
    ]
    task_analyses = [analyze_task_dir(d) for d in task_dirs]

    total_tasks = len(task_analyses)
    passed_tasks = [t for t in task_analyses if t["reward_bool"]]
    failed_tasks = [t for t in task_analyses if not t["reward_bool"]]

    pass_rate = (len(passed_tasks) / total_tasks * 100) if total_tasks > 0 else 0.0

    failure_groups: Dict[str, List[Dict[str, Any]]] = {}
    for t in failed_tasks:
        failure_groups.setdefault(t["failure_category"], []).append(t)

    # Sort categories for stable, deterministic reporting order
    sorted_failure_groups = dict(
        sorted(failure_groups.items(), key=lambda item: category_sort_key(item[0]))
    )

    tool_usage: Counter[str] = Counter()
    for t in task_analyses:
        tool_usage.update(t["tool_calls"])

    summary = {
        "job_id": job_dir.name,
        "total_tasks": total_tasks,
        "passed_tasks_count": len(passed_tasks),
        "failed_tasks_count": len(failed_tasks),
        "pass_rate_pct": round(pass_rate, 2),
        "failure_categories": {k: len(v) for k, v in sorted_failure_groups.items()},
        "tool_usage_counts": dict(tool_usage),
        "notes": [
            "heuristic_error_observations is keyword-based and can include false positives",
            "Prefer harbor-harness-improvement-loop for evidence-backed BM→fix cycles",
        ],
        "failed_tasks_detail": failed_tasks,
        "passed_tasks_detail": passed_tasks,
    }

    print("\nSummary:")
    print(f"  Total Trials : {total_tasks}")
    print(f"  Passed       : {len(passed_tasks)} ({pass_rate:.1f}%)")
    print(f"  Failed       : {len(failed_tasks)}")

    print("\nFailure Categories:")
    for cat, tasks in sorted_failure_groups.items():
        print(f"  - {cat}: {len(tasks)} tasks")
        for t in tasks:
            exc_str = f", Exception: {t['exception']}" if t["exception"] else ""
            heur = t["heuristic_error_observations"]
            heur_str = f", heuristic_obs_errors: {heur}" if heur else ""
            repeats = t["repeated_adjacent_calls"]
            rep_str = f", repeated_calls: {repeats}" if repeats else ""
            print(
                f"      • {t['task_name']} (turns: {t['turns_count']}, "
                f"tools: {t['tool_calls_count']} via {t['tool_source']}{exc_str}{heur_str}{rep_str})"
            )
            if args.verbose and t["tool_calls"]:
                print(f"        tools: {', '.join(t['tool_calls'][:20])}")

    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        if out_path.suffix == ".json":
            out_path.write_text(
                json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
            )
        else:
            md = [
                f"# Harness Analysis Report: `{job_dir.name}`\n",
                f"- **Total Trials**: {total_tasks}",
                f"- **Pass Rate**: {pass_rate:.1f}% ({len(passed_tasks)}/{total_tasks})",
                f"- **Failures**: {len(failed_tasks)}\n",
                "> Observation error hits are **heuristics**, not verified tool failures. "
                "For BM→fix cycles use `harbor-harness-improvement-loop`.\n",
                "## Failure Categorization\n",
            ]
            for cat, tasks in sorted_failure_groups.items():
                md.append(f"### {cat} ({len(tasks)} trials)")
                for t in tasks:
                    md.append(
                        f"- **{t['task_name']}**: Turns={t['turns_count']}, "
                        f"ToolCalls={t['tool_calls_count']} ({t['tool_source']}), "
                        f"RepeatedCalls={t['repeated_adjacent_calls']}, "
                        f"HeuristicObsErrors={t['heuristic_error_observations']}, "
                        f"Exception=`{t['exception']}`"
                    )
                md.append("")

            md.append("## Harness Bottlenecks & Recommendations\n")
            if "TIMEOUT_EXCEEDED" in sorted_failure_groups:
                md.append("### 1. Timeout / Execution Bottleneck")
                md.append("- **Observed**: Tasks timed out before completion.")
                md.append(
                    "- **Action Plan**: Inspect tool execution timeouts and retry/"
                    "circuit-breaker patterns.\n"
                )
            if "NETWORK_API_ERROR" in sorted_failure_groups:
                md.append("### 2. Network / API Connection Failures")
                md.append("- **Observed**: Network disconnection or LLM provider API errors occurred.")
                md.append(
                    "- **Action Plan**: Check network proxy, API rate limits, or retry logic.\n"
                )
            if "INITIALIZATION_OR_EARLY_ABORT" in sorted_failure_groups:
                md.append("### 3. Early Abort / 0-Turn Failures")
                md.append(
                    "- **Observed**: Agent failed to execute any turns before termination."
                )
                md.append(
                    "- **Action Plan**: Check adapter setup, model authorization, "
                    "or initial prompt parsing.\n"
                )
            if "TOOL_EXECUTION_ERROR" in sorted_failure_groups:
                md.append("### 4. Explicit Tool Errors")
                md.append(
                    "- **Observed**: Structured tool error fields were present "
                    "(not observation-keyword heuristics)."
                )
                md.append(
                    "- **Action Plan**: Review tool schemas/handlers under "
                    "`src-tauri/src/mcp/builtin/`.\n"
                )
            if "AGENT_LOOP_OR_STUCK" in sorted_failure_groups:
                md.append("### 5. Repeated Tool Loop")
                md.append(
                    "- **Observed**: Repeated adjacent identical tool calls (loop detected)."
                )
                md.append(
                    "- **Action Plan**: Inspect circuit breaker / natural recovery guidance.\n"
                )
            if "HIGH_TURN_COUNT" in sorted_failure_groups:
                md.append("### 6. High Turn Count")
                md.append(
                    "- **Observed**: >30 turns without repeated-call evidence of a tight loop."
                )
                md.append(
                    "- **Action Plan**: Manual ATIF inspection — do not assume stuckness.\n"
                )

            out_path.write_text("\n".join(md), encoding="utf-8")
        print(f"\nSaved analysis report to {out_path}")


if __name__ == "__main__":
    main()
