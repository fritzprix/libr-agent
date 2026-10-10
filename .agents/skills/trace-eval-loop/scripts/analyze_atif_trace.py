#!/usr/bin/env python3
"""Analyze session-exported ATIF-v1.7 trajectories for tool/harness eval loops.

Accepts one or more ATIF JSON files, or directories containing
`*_trajectory.json` / `trajectory.json`.

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
from typing import Any, Iterable

ERROR_SIGNAL = re.compile(
    r"\b(error|failed|failure|invalid|not found|permission denied|"
    r"timed? out|timeout|traceback|exception|unknown tool)\b",
    re.IGNORECASE,
)

UNKNOWN_TOOL = re.compile(r"unknown tool\s*:\s*(\S+)", re.IGNORECASE)

DEFAULT_OVERSIZE_CHARS = 8000
TRAJECTORY_NAME_GLOBS = ("*_trajectory.json", "trajectory.json")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Analyze session-exported ATIF-v1.7 trajectories."
    )
    parser.add_argument(
        "inputs",
        nargs="+",
        help="ATIF JSON file(s) and/or directories to scan",
    )
    parser.add_argument(
        "--output",
        help="Write machine-readable JSON summary to this path",
    )
    parser.add_argument(
        "--markdown",
        help="Write human-readable Markdown summary to this path",
    )
    parser.add_argument(
        "--oversize-chars",
        type=int,
        default=DEFAULT_OVERSIZE_CHARS,
        help=f"Observation content length threshold (default {DEFAULT_OVERSIZE_CHARS})",
    )
    parser.add_argument(
        "--sequence-limit",
        type=int,
        default=80,
        help="Max tool-sequence events to include per trajectory (default 80)",
    )
    return parser.parse_args(argv)


def load_json(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return None, f"{path}: {exc}"
    if not isinstance(payload, dict):
        return None, f"{path}: expected a JSON object"
    return payload, None


def discover_paths(inputs: Iterable[str]) -> list[Path]:
    found: list[Path] = []
    seen: set[Path] = set()
    for raw in inputs:
        path = Path(raw).expanduser().resolve()
        candidates: list[Path] = []
        if path.is_file():
            candidates.append(path)
        elif path.is_dir():
            for pattern in TRAJECTORY_NAME_GLOBS:
                candidates.extend(sorted(path.rglob(pattern)))
        else:
            print(f"Warning: path not found: {path}", file=sys.stderr)
            continue
        for candidate in candidates:
            if candidate.suffix.lower() != ".json":
                continue
            resolved = candidate.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            found.append(resolved)
    return found


def number(value: Any) -> float | None:
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


def canonical_call(call: dict[str, Any]) -> str:
    name = call.get("function_name") or call.get("name") or "<unknown>"
    arguments = call.get("arguments")
    try:
        encoded = json.dumps(arguments, sort_keys=True, ensure_ascii=False)
    except (TypeError, ValueError):
        encoded = repr(arguments)
    return f"{name}:{encoded}"


def observation_entries(step: dict[str, Any]) -> Iterable[dict[str, Any]]:
    """Yield observation result dicts (Harbor/session ATIF: observation.results[])."""
    observation = step.get("observation")
    if not isinstance(observation, dict):
        return
    results = observation.get("results")
    if isinstance(results, list):
        for result in results:
            if isinstance(result, dict):
                yield result
        return
    # Legacy / simplified docs shape: observation.content (+ optional tool_call_id)
    if "content" in observation:
        yield {
            "source_call_id": observation.get("tool_call_id")
            or observation.get("source_call_id"),
            "content": observation.get("content"),
        }


def content_to_text(content: Any) -> str:
    if isinstance(content, str):
        return content
    if content is None:
        return ""
    try:
        return json.dumps(content, ensure_ascii=False)
    except (TypeError, ValueError):
        return str(content)


def truncate(text: str, limit: int = 160) -> str:
    compact = " ".join(text.split())
    if len(compact) <= limit:
        return compact
    return compact[: limit - 1] + "…"


def analyze_trajectory(
    path: Path,
    payload: dict[str, Any],
    *,
    oversize_chars: int,
    sequence_limit: int,
) -> dict[str, Any]:
    steps = payload.get("steps")
    if not isinstance(steps, list):
        steps = []

    agent = payload.get("agent")
    if not isinstance(agent, dict):
        agent = {}
    metrics = payload.get("final_metrics")
    if not isinstance(metrics, dict):
        metrics = {}

    agent_steps = 0
    user_steps = 0
    env_steps = 0
    tool_calls = 0
    repeated_adjacent_calls = 0
    heuristic_error_observations = 0
    oversized_observations = 0
    orphan_observation_hints = 0
    per_tool: Counter[str] = Counter()
    unknown_tools: Counter[str] = Counter()
    previous_call: str | None = None
    sequence: list[dict[str, Any]] = []
    first_user_message: str | None = None
    flags: list[str] = []

    schema = payload.get("schema_version")
    if schema != "ATIF-v1.7":
        flags.append(f"unexpected_schema_version:{schema!r}")

    for raw_step in steps:
        if not isinstance(raw_step, dict):
            continue
        step_id = raw_step.get("step_id")
        source = str(raw_step.get("source") or "")
        message = raw_step.get("message")
        message_text = message if isinstance(message, str) else ""

        if source == "agent":
            agent_steps += 1
        elif source == "user":
            user_steps += 1
            if first_user_message is None and message_text.strip():
                first_user_message = message_text.strip()
        elif source == "environment":
            env_steps += 1

        if (
            message_text.startswith("(no LibrAgent messages harvested")
            or message_text.startswith("(orphaned LibrAgent tool results)")
        ):
            flags.append(f"placeholder_step:{step_id}:{truncate(message_text, 80)}")

        calls = raw_step.get("tool_calls")
        if isinstance(calls, list):
            for raw_call in calls:
                if not isinstance(raw_call, dict):
                    continue
                tool_calls += 1
                name = str(
                    raw_call.get("function_name") or raw_call.get("name") or "<unknown>"
                )
                per_tool[name] += 1
                call_key = canonical_call(raw_call)
                if call_key == previous_call:
                    repeated_adjacent_calls += 1
                previous_call = call_key
                if len(sequence) < sequence_limit:
                    sequence.append(
                        {
                            "kind": "tool_call",
                            "step_id": step_id,
                            "tool_call_id": raw_call.get("tool_call_id"),
                            "function_name": name,
                            "arguments_preview": truncate(
                                content_to_text(raw_call.get("arguments")), 120
                            ),
                        }
                    )

        for result in observation_entries(raw_step):
            text = content_to_text(result.get("content"))
            source_call_id = result.get("source_call_id")
            if not source_call_id and "(orphaned" in message_text:
                orphan_observation_hints += 1
            if len(text) >= oversize_chars:
                oversized_observations += 1
            if ERROR_SIGNAL.search(text):
                heuristic_error_observations += 1
                match = UNKNOWN_TOOL.search(text)
                if match:
                    unknown_tools[match.group(1)] += 1
            if len(sequence) < sequence_limit:
                sequence.append(
                    {
                        "kind": "observation",
                        "step_id": step_id,
                        "source_call_id": source_call_id,
                        "chars": len(text),
                        "heuristic_error": bool(ERROR_SIGNAL.search(text)),
                        "preview": truncate(text, 160),
                    }
                )

    if not steps:
        flags.append("empty_steps")
    if tool_calls == 0 and agent_steps > 0:
        flags.append("agent_steps_without_tool_calls")
    if metrics.get("total_prompt_tokens") is None:
        flags.append("missing_final_metrics_prompt_tokens")

    return {
        "path": str(path),
        "schema_version": schema,
        "session_id": payload.get("session_id"),
        "agent_name": agent.get("name"),
        "agent_version": agent.get("version"),
        "model_name": agent.get("model_name"),
        "first_user_message_preview": truncate(first_user_message or "", 240) or None,
        "steps": len(steps),
        "user_steps": user_steps,
        "agent_steps": agent_steps,
        "environment_steps": env_steps,
        "tool_calls": tool_calls,
        "per_tool": dict(sorted(per_tool.items())),
        "repeated_adjacent_calls": repeated_adjacent_calls,
        "heuristic_error_observations": heuristic_error_observations,
        "oversized_observations": oversized_observations,
        "oversize_chars_threshold": oversize_chars,
        "orphan_observation_hints": orphan_observation_hints,
        "unknown_tools_heuristic": dict(sorted(unknown_tools.items())),
        "prompt_tokens": number(metrics.get("total_prompt_tokens")),
        "completion_tokens": number(metrics.get("total_completion_tokens")),
        "cached_tokens": number(metrics.get("total_cached_tokens")),
        "final_metrics_total_steps": metrics.get("total_steps"),
        "flags": flags,
        "sequence": sequence,
        "sequence_truncated": tool_calls + heuristic_error_observations > sequence_limit,
        "notes": [
            "heuristic_error_observations are keyword hits, not verified failures",
            "model_name is inventory-only; do not use as a root-cause owner",
        ],
    }


def aggregate(summaries: list[dict[str, Any]], parse_errors: list[str]) -> dict[str, Any]:
    per_tool: Counter[str] = Counter()
    for item in summaries:
        per_tool.update(item.get("per_tool") or {})
    return {
        "trajectory_count": len(summaries),
        "invalid_artifacts": len(parse_errors),
        "parse_errors": parse_errors,
        "totals": {
            "steps": sum(int(item["steps"]) for item in summaries),
            "tool_calls": sum(int(item["tool_calls"]) for item in summaries),
            "repeated_adjacent_calls": sum(
                int(item["repeated_adjacent_calls"]) for item in summaries
            ),
            "heuristic_error_observations": sum(
                int(item["heuristic_error_observations"]) for item in summaries
            ),
            "oversized_observations": sum(
                int(item["oversized_observations"]) for item in summaries
            ),
        },
        "per_tool": dict(sorted(per_tool.items(), key=lambda kv: (-kv[1], kv[0]))),
        "trajectories": summaries,
    }


def render_markdown(report: dict[str, Any]) -> str:
    lines: list[str] = []
    lines.append("# Session ATIF trace summary")
    lines.append("")
    lines.append(
        f"- Trajectories: **{report['trajectory_count']}** "
        f"(invalid: {report['invalid_artifacts']})"
    )
    totals = report["totals"]
    lines.append(
        f"- Totals — steps: {totals['steps']}, tool_calls: {totals['tool_calls']}, "
        f"adjacent_repeats: {totals['repeated_adjacent_calls']}, "
        f"heuristic_errors: {totals['heuristic_error_observations']}, "
        f"oversized_obs: {totals['oversized_observations']}"
    )
    lines.append("")
    lines.append(
        "> Heuristic error observations are keyword hits only — not verified "
        "tool failures. Don't-care for root cause: model, serving engine."
    )
    lines.append("")
    if report["per_tool"]:
        lines.append("## Tools (all trajectories)")
        lines.append("")
        for name, count in list(report["per_tool"].items())[:40]:
            lines.append(f"- `{name}`: {count}")
        lines.append("")
    if report["parse_errors"]:
        lines.append("## Parse errors")
        lines.append("")
        for err in report["parse_errors"]:
            lines.append(f"- {err}")
        lines.append("")

    for idx, item in enumerate(report["trajectories"], start=1):
        lines.append(f"## Trajectory {idx}: `{Path(item['path']).name}`")
        lines.append("")
        lines.append(f"- Path: `{item['path']}`")
        lines.append(f"- Schema: `{item.get('schema_version')}`")
        lines.append(f"- Session: `{item.get('session_id')}`")
        lines.append(
            f"- Agent: {item.get('agent_name')} {item.get('agent_version')} "
            f"(model inventory: `{item.get('model_name')}`)"
        )
        if item.get("first_user_message_preview"):
            lines.append(f"- First user: {item['first_user_message_preview']}")
        lines.append(
            f"- Steps: {item['steps']} (user={item['user_steps']}, "
            f"agent={item['agent_steps']}, env={item['environment_steps']})"
        )
        lines.append(
            f"- Tool calls: {item['tool_calls']}; adjacent repeats: "
            f"{item['repeated_adjacent_calls']}; heuristic errors: "
            f"{item['heuristic_error_observations']}; oversized: "
            f"{item['oversized_observations']} (≥{item['oversize_chars_threshold']})"
        )
        lines.append(
            f"- Tokens: prompt={item.get('prompt_tokens')}, "
            f"completion={item.get('completion_tokens')}, "
            f"cached={item.get('cached_tokens')}"
        )
        if item.get("flags"):
            lines.append(f"- Flags: {', '.join(item['flags'])}")
        if item.get("unknown_tools_heuristic"):
            lines.append(
                f"- Unknown-tool heuristic hits: {item['unknown_tools_heuristic']}"
            )
        if item.get("per_tool"):
            lines.append("- Per-tool:")
            for name, count in item["per_tool"].items():
                lines.append(f"  - `{name}`: {count}")
        if item.get("sequence"):
            lines.append("- Sequence (truncated):")
            for event in item["sequence"]:
                if event["kind"] == "tool_call":
                    lines.append(
                        f"  - call step={event.get('step_id')} "
                        f"`{event.get('function_name')}` "
                        f"args={event.get('arguments_preview')}"
                    )
                else:
                    err = " ERR?" if event.get("heuristic_error") else ""
                    lines.append(
                        f"  - obs step={event.get('step_id')} "
                        f"chars={event.get('chars')}{err} "
                        f"{event.get('preview')}"
                    )
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    paths = discover_paths(args.inputs)
    if not paths:
        print("No ATIF trajectory JSON files found.", file=sys.stderr)
        return 1

    summaries: list[dict[str, Any]] = []
    parse_errors: list[str] = []
    for path in paths:
        payload, error = load_json(path)
        if error:
            parse_errors.append(error)
            continue
        assert payload is not None
        summaries.append(
            analyze_trajectory(
                path,
                payload,
                oversize_chars=args.oversize_chars,
                sequence_limit=args.sequence_limit,
            )
        )

    report = aggregate(summaries, parse_errors)
    text = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)

    if args.markdown:
        md_path = Path(args.markdown)
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(render_markdown(report), encoding="utf-8")

    return 0 if summaries else 1


if __name__ == "__main__":
    raise SystemExit(main())
