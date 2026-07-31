#!/usr/bin/env python3
"""Summarize Harbor result, reward, and ATIF trajectory artifacts.

This script reports descriptive metrics. Observation error detection is a
keyword heuristic and must not be treated as a verified tool failure.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
from collections import Counter
from pathlib import Path
from typing import Any, Iterable


ERROR_SIGNAL = re.compile(
    r"\b(error|failed|failure|invalid|not found|permission denied|"
    r"timed? out|timeout|traceback|exception)\b",
    re.IGNORECASE,
)


def load_json(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return None, f"{path}: {exc}"
    if not isinstance(payload, dict):
        return None, f"{path}: expected a JSON object"
    return payload, None


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


def distribution(values: Iterable[float]) -> dict[str, float | int | None]:
    items = list(values)
    if not items:
        return {
            "count": 0,
            "sum": None,
            "min": None,
            "median": None,
            "mean": None,
            "max": None,
        }
    return {
        "count": len(items),
        "sum": sum(items),
        "min": min(items),
        "median": statistics.median(items),
        "mean": statistics.fmean(items),
        "max": max(items),
    }


def canonical_call(call: dict[str, Any]) -> str:
    name = call.get("function_name") or call.get("name") or "<unknown>"
    arguments = call.get("arguments")
    try:
        encoded = json.dumps(arguments, sort_keys=True, ensure_ascii=False)
    except (TypeError, ValueError):
        encoded = repr(arguments)
    return f"{name}:{encoded}"


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


def summarize_trajectory(path: Path, payload: dict[str, Any]) -> dict[str, Any]:
    steps = payload.get("steps")
    if not isinstance(steps, list):
        steps = []

    agent_steps = 0
    tool_calls = 0
    repeated_adjacent_calls = 0
    heuristic_error_observations = 0
    per_tool: Counter[str] = Counter()
    previous_call: str | None = None

    for raw_step in steps:
        if not isinstance(raw_step, dict):
            continue
        if raw_step.get("source") == "agent":
            agent_steps += 1
        calls = raw_step.get("tool_calls")
        if isinstance(calls, list):
            for raw_call in calls:
                if not isinstance(raw_call, dict):
                    continue
                tool_calls += 1
                name = raw_call.get("function_name") or raw_call.get("name")
                per_tool[str(name or "<unknown>")] += 1
                call_key = canonical_call(raw_call)
                if call_key == previous_call:
                    repeated_adjacent_calls += 1
                previous_call = call_key
        for text in observation_texts(raw_step):
            if ERROR_SIGNAL.search(text):
                heuristic_error_observations += 1

    metrics = payload.get("final_metrics")
    if not isinstance(metrics, dict):
        metrics = {}
    agent = payload.get("agent")
    if not isinstance(agent, dict):
        agent = {}

    return {
        "path": str(path),
        "schema_version": payload.get("schema_version"),
        "session_id": payload.get("session_id"),
        "agent_name": agent.get("name"),
        "agent_version": agent.get("version"),
        "model_name": agent.get("model_name"),
        "steps": len(steps),
        "agent_steps": agent_steps,
        "tool_calls": tool_calls,
        "per_tool": dict(sorted(per_tool.items())),
        "repeated_adjacent_calls": repeated_adjacent_calls,
        "heuristic_error_observations": heuristic_error_observations,
        "prompt_tokens": number(metrics.get("total_prompt_tokens")),
        "completion_tokens": number(metrics.get("total_completion_tokens")),
        "cached_tokens": number(metrics.get("total_cached_tokens")),
    }


def collect_rewards(root: Path, parse_errors: list[str]) -> list[dict[str, Any]]:
    rewards: list[dict[str, Any]] = []
    for path in sorted(root.rglob("reward.txt")):
        if path.parent.name != "verifier":
            continue
        try:
            raw = path.read_text(encoding="utf-8").strip()
        except (OSError, UnicodeError) as exc:
            parse_errors.append(f"{path}: {exc}")
            continue
        rewards.append({"path": str(path), "raw": raw, "value": number(raw)})
    return rewards


def collect_evals(root: Path, parse_errors: list[str]) -> list[dict[str, Any]]:
    evals: list[dict[str, Any]] = []
    for path in sorted(root.rglob("result.json")):
        payload, error = load_json(path)
        if error:
            parse_errors.append(error)
            continue
        assert payload is not None
        stats = payload.get("stats")
        if not isinstance(stats, dict):
            continue
        raw_evals = stats.get("evals")
        if not isinstance(raw_evals, dict):
            continue
        for name, raw_eval in raw_evals.items():
            if not isinstance(raw_eval, dict):
                continue
            metrics = raw_eval.get("metrics")
            first_metric = (
                metrics[0]
                if isinstance(metrics, list) and metrics and isinstance(metrics[0], dict)
                else {}
            )
            evals.append(
                {
                    "path": str(path),
                    "name": str(name),
                    "mean": number(first_metric.get("mean")),
                    "n_trials": number(raw_eval.get("n_trials")),
                    "n_errors": number(raw_eval.get("n_errors")),
                }
            )
    return evals


def summarize_root(root: Path) -> dict[str, Any]:
    parse_errors: list[str] = []
    trajectories: list[dict[str, Any]] = []
    result_paths = list(root.rglob("result.json"))
    for path in sorted(root.rglob("trajectory.json")):
        payload, error = load_json(path)
        if error:
            parse_errors.append(error)
            continue
        assert payload is not None
        trajectories.append(summarize_trajectory(path, payload))

    rewards = collect_rewards(root, parse_errors)
    evals = collect_evals(root, parse_errors)
    reward_values = [item["value"] for item in rewards if item["value"] is not None]
    prompt = [item["prompt_tokens"] for item in trajectories if item["prompt_tokens"] is not None]
    completion = [
        item["completion_tokens"]
        for item in trajectories
        if item["completion_tokens"] is not None
    ]
    cached = [item["cached_tokens"] for item in trajectories if item["cached_tokens"] is not None]
    turns = [float(item["agent_steps"]) for item in trajectories]
    calls = [float(item["tool_calls"]) for item in trajectories]
    per_tool: Counter[str] = Counter()
    model_counts: Counter[str] = Counter()
    for item in trajectories:
        per_tool.update(item["per_tool"])
        model_counts[str(item["model_name"] or "<missing>")] += 1

    prompt_total = sum(prompt)
    cached_total = sum(cached)
    return {
        "root": str(root),
        "artifact_counts": {
            "result_json": len(result_paths),
            "reward_txt": len(rewards),
            "trajectory_json": len(trajectories),
            "invalid_artifacts": len(parse_errors),
            "trajectories_without_prompt_tokens": sum(
                1 for item in trajectories if item["prompt_tokens"] is None
            ),
        },
        "reward": distribution(reward_values),
        "prompt_tokens": distribution(prompt),
        "completion_tokens": distribution(completion),
        "cached_tokens": distribution(cached),
        "cache_ratio_of_prompt_total": (
            cached_total / prompt_total if prompt_total > 0 else None
        ),
        "agent_turns": distribution(turns),
        "tool_calls": distribution(calls),
        "repeated_adjacent_calls": sum(
            item["repeated_adjacent_calls"] for item in trajectories
        ),
        "heuristic_error_observations": sum(
            item["heuristic_error_observations"] for item in trajectories
        ),
        "per_tool_calls": dict(sorted(per_tool.items())),
        "model_counts": dict(sorted(model_counts.items())),
        "evals": evals,
        "rewards": rewards,
        "trajectories": trajectories,
        "parse_errors": parse_errors,
        "caveats": [
            "heuristic_error_observations is keyword-based and can include false positives",
            "repeated_adjacent_calls only detects exact adjacent name+argument repeats",
            "descriptive differences do not establish causality",
        ],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("roots", nargs="+", type=Path, help="Harbor job directories")
    parser.add_argument("--output", type=Path, help="Write combined JSON here")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    missing = [str(root) for root in args.roots if not root.is_dir()]
    if missing:
        raise SystemExit(f"Not a directory: {', '.join(missing)}")

    report = {
        "schema_version": 1,
        "runs": [summarize_root(root.resolve()) for root in args.roots],
    }
    encoded = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
        print(args.output)
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
