#!/usr/bin/env python3
"""
trace-eval-loop: Automated Agent Trajectory & Trace Diagnostic Analyzer

Parses agent trajectory files (ATIF-v1.x, LibrAgent .trace.json, or Session Messages export)
to extract execution metrics, tool telemetry, error patterns, polling waste, sub-session lineage,
and user friction points.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# Ensure UTF-8 output on Windows consoles
if sys.stdout and hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass


@dataclass
class ToolCallRecord:
    step_id: int
    call_id: str
    function_name: str
    arguments: Dict[str, Any]
    observation_text: str = ""
    observation_bytes: int = 0
    is_error: bool = False
    error_summary: str = ""
    exit_code: Optional[int] = None


@dataclass
class UserTurnRecord:
    step_id: int
    message: str
    friction_signals: List[str] = field(default_factory=list)


@dataclass
class SubsessionSummary:
    session_id: str
    spawns: int = 0
    messages_sent: int = 0
    checks_polled: int = 0
    errors: int = 0


@dataclass
class TraceAnalysisReport:
    format_type: str
    session_id: str
    agent_info: Dict[str, Any]
    total_steps: int
    user_turns_count: int
    agent_turns_count: int
    prompt_tokens: int
    completion_tokens: int
    cached_tokens: int
    cache_hit_ratio: float
    tool_counts: Counter
    tool_errors: Counter
    tool_bytes: Counter
    error_categories: Counter
    repeated_call_loops: List[Dict[str, Any]]
    polling_check_count: int
    subsessions: Dict[str, SubsessionSummary]
    user_turns: List[UserTurnRecord]
    tool_calls: List[ToolCallRecord]
    findings: List[Dict[str, Any]] = field(default_factory=list)


FRICTION_KEYWORDS = [
    "왜", "안돼", "링크", "출처", "누락", "다시", "에러", "실패", "틀렸",
    "제외", "축약", "이해", "수정", "아니", "병신", "젠장", "이상해", "멈춤",
    "why", "missing", "link", "url", "fail", "error", "wrong", "fix", "stop",
]


def detect_friction(message: str) -> List[str]:
    hits = []
    lower = message.lower()
    for kw in FRICTION_KEYWORDS:
        if kw in lower:
            hits.append(kw)
    return hits


def parse_atif_trajectory(data: Dict[str, Any]) -> TraceAnalysisReport:
    session_id = data.get("session_id", "unknown")
    agent_info = data.get("agent", {})
    steps = data.get("steps", [])

    final_metrics = data.get("final_metrics", {})
    prompt_tokens = final_metrics.get("total_prompt_tokens", 0)
    completion_tokens = final_metrics.get("total_completion_tokens", 0)
    cached_tokens = final_metrics.get("total_cached_tokens", 0)

    # If final_metrics is missing, aggregate from steps
    if prompt_tokens == 0:
        for s in steps:
            m = s.get("metrics", {})
            prompt_tokens += m.get("prompt_tokens", 0)
            completion_tokens += m.get("completion_tokens", 0)
            cached_tokens += m.get("cached_tokens", 0)

    cache_ratio = (cached_tokens / prompt_tokens * 100.0) if prompt_tokens > 0 else 0.0

    tool_counts: Counter = Counter()
    tool_errors: Counter = Counter()
    tool_bytes: Counter = Counter()
    error_categories: Counter = Counter()
    all_tool_calls: List[ToolCallRecord] = []
    user_turns: List[UserTurnRecord] = []
    subsessions: Dict[str, SubsessionSummary] = defaultdict(lambda: SubsessionSummary(session_id=""))

    user_count = 0
    agent_count = 0

    tool_call_history: List[Tuple[int, str, str]] = []  # (step_id, fn, args_repr)

    for step in steps:
        step_id = step.get("step_id", 0)
        source = step.get("source", "")
        message = step.get("message", "")

        if source == "user":
            user_count += 1
            frictions = detect_friction(message)
            user_turns.append(UserTurnRecord(step_id=step_id, message=message, friction_signals=frictions))
            continue
        elif source == "agent":
            agent_count += 1

        tcs = step.get("tool_calls", [])
        obs = step.get("observation", {})
        results = obs.get("results", [])
        res_by_id = {r.get("source_call_id"): r for r in results if isinstance(r, dict)}

        for tc in tcs:
            fn = tc.get("function_name", "unknown")
            cid = tc.get("tool_call_id", "")
            args = tc.get("arguments", {})
            tool_counts[fn] += 1

            # Sub-session tracking
            if fn.startswith("agent__"):
                target_sub = args.get("sessionId") or args.get("childSessionId") or ""
                if fn == "agent__spawnSession":
                    # We might get target_sub from observation or args
                    pass
                elif target_sub:
                    if target_sub not in subsessions:
                        subsessions[target_sub] = SubsessionSummary(session_id=target_sub)
                    if fn == "agent__messageToSession":
                        subsessions[target_sub].messages_sent += 1
                    elif fn == "agent__checkSession":
                        subsessions[target_sub].checks_polled += 1

            args_repr = json.dumps(args, sort_keys=True, ensure_ascii=False)
            tool_call_history.append((step_id, fn, args_repr))

            res = res_by_id.get(cid, {})
            content = str(res.get("content", ""))
            obs_bytes = len(content.encode("utf-8"))
            tool_bytes[fn] += obs_bytes

            is_err = False
            err_summary = ""
            exit_code = None

            # Check exit codes
            exit_code_match = re.search(r"exit code:\s*(-?\d+)", content, re.IGNORECASE)
            if exit_code_match:
                exit_code = int(exit_code_match.group(1))
                if exit_code != 0:
                    is_err = True

            if "✗" in content or "Error:" in content or "failed with" in content or "timed out" in content.lower():
                is_err = True

            if is_err:
                tool_errors[fn] += 1
                first_line = content.strip().split("\n")[0][:100]
                if exit_code == 9009 or "9009" in content:
                    cat = f"{fn}: Command Not Found (9009)"
                elif exit_code == 1:
                    cat = f"{fn}: Exit Code 1"
                elif exit_code == 2:
                    cat = f"{fn}: Exit Code 2"
                elif "timed out" in content.lower():
                    cat = f"{fn}: Timeout / Polling Delay"
                elif "not found" in content.lower():
                    cat = f"{fn}: Resource Not Found"
                elif "already exists" in content.lower():
                    cat = f"{fn}: Conflict / Already Exists"
                elif "interrupted" in content.lower() or "cancelled" in content.lower():
                    cat = f"{fn}: Cancelled / Interrupted"
                elif "old_string" in content:
                    cat = f"{fn}: strReplace Target Mismatch"
                else:
                    cat = f"{fn}: {first_line}"
                error_categories[cat] += 1
                err_summary = first_line

                # Subsession error recording
                if fn.startswith("agent__") and (args.get("sessionId") or args.get("childSessionId")):
                    sid_key = args.get("sessionId") or args.get("childSessionId")
                    if sid_key in subsessions:
                        subsessions[sid_key].errors += 1

            all_tool_calls.append(
                ToolCallRecord(
                    step_id=step_id,
                    call_id=cid,
                    function_name=fn,
                    arguments=args,
                    observation_text=content[:500],
                    observation_bytes=obs_bytes,
                    is_error=is_err,
                    error_summary=err_summary,
                    exit_code=exit_code,
                )
            )

    # Detect loops (>= 3 consecutive identical tool calls)
    repeated_loops = []
    consecutive_fn = ""
    consecutive_count = 0
    start_step = 0
    for sid, fn, args_repr in tool_call_history:
        if fn == consecutive_fn:
            consecutive_count += 1
        else:
            if consecutive_count >= 3:
                repeated_loops.append({
                    "function_name": consecutive_fn,
                    "count": consecutive_count,
                    "start_step": start_step,
                    "end_step": sid - 1,
                })
            consecutive_fn = fn
            consecutive_count = 1
            start_step = sid

    if consecutive_count >= 3:
        repeated_loops.append({
            "function_name": consecutive_fn,
            "count": consecutive_count,
            "start_step": start_step,
            "end_step": steps[-1].get("step_id", start_step) if steps else start_step,
        })

    polling_check_count = tool_counts.get("agent__checkSession", 0)

    # Derive high-level findings
    findings = []
    if polling_check_count > 30:
        findings.append({
            "level": "P0",
            "type": "HARNESS_POLLING_STORM",
            "title": f"Excessive session polling detected: {polling_check_count} calls to agent__checkSession",
            "detail": f"Active polling inflated turn counts and context token bloat. Introduce async push notifications, longer polling timeout defaults, or exponential backoff."
        })

    if prompt_tokens > 20_000_000:
        findings.append({
            "level": "P0",
            "type": "TOKEN_EXPLOSION",
            "title": f"Extremely high token consumption: {prompt_tokens:,} prompt tokens",
            "detail": f"Session consumed {prompt_tokens:,} prompt tokens across {len(steps)} steps. Indicates massive cumulative history accumulation without observation pruning or compaction."
        })

    if tool_errors.get("workspace__runPowerShell", 0) > 10:
        findings.append({
            "level": "P1",
            "type": "POWERSHELL_ENVIRONMENT_FAILURES",
            "title": f"Shell script instability: {tool_errors['workspace__runPowerShell']} failed executions",
            "detail": "Frequent exit code 9009 (command not found) or python runtime failures. Requires environment preflight verification or dedicated built-in tools instead of raw shell."
        })

    if tool_errors.get("agent__createAgent", 0) > 0:
        findings.append({
            "level": "P1",
            "type": "NON_IDEMPOTENT_TOOL",
            "title": "Idempotency breach in agent__createAgent",
            "detail": "Assistant creation failed because assistant already exists (409). Builtin tools should support get-or-create or upsert semantics."
        })

    if any("Resource Not Found" in cat for cat in error_categories if "agent__" in cat):
        findings.append({
            "level": "P0",
            "type": "SESSION_ID_SSOT_BREACH",
            "title": "Subsession ID mismatch or truncation",
            "detail": "Agent attempted to query child sessions using truncated or invalid session IDs. Strict single ID (SSOT) enforcement in prompts and tool schemas is required."
        })

    return TraceAnalysisReport(
        format_type="ATIF-v1.x",
        session_id=session_id,
        agent_info=agent_info,
        total_steps=len(steps),
        user_turns_count=user_count,
        agent_turns_count=agent_count,
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
        cached_tokens=cached_tokens,
        cache_hit_ratio=cache_ratio,
        tool_counts=tool_counts,
        tool_errors=tool_errors,
        tool_bytes=tool_bytes,
        error_categories=error_categories,
        repeated_call_loops=repeated_loops,
        polling_check_count=polling_check_count,
        subsessions=dict(subsessions),
        user_turns=user_turns,
        tool_calls=all_tool_calls,
        findings=findings,
    )


def parse_trace_file(file_path: Path) -> TraceAnalysisReport:
    with open(file_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    if isinstance(data, dict) and ("schema_version" in data or "steps" in data):
        return parse_atif_trajectory(data)
    elif isinstance(data, list):
        # Raw messages format
        synthetic = {
            "schema_version": "RAW-MESSAGES",
            "session_id": file_path.stem,
            "agent": {},
            "steps": [],
        }
        for idx, item in enumerate(data):
            role = item.get("role", "user" if item.get("type") == "user" else "agent")
            synthetic["steps"].append({
                "step_id": idx + 1,
                "source": "user" if role == "user" else "agent",
                "message": item.get("content", ""),
                "tool_calls": item.get("tool_calls", []),
                "observation": {"results": item.get("tool_results", [])},
            })
        return parse_atif_trajectory(synthetic)
    else:
        raise ValueError(f"Unrecognized trace format in {file_path}")


def generate_markdown_report(report: TraceAnalysisReport, trace_filename: str, top_n: int = 15) -> str:
    lines = []
    lines.append(f"# 🔍 Trajectory Evaluation Report: `{trace_filename}`")
    lines.append("")
    lines.append("## 1. Executive Summary")
    lines.append("")
    lines.append(f"- **Session ID**: `{report.session_id}`")
    lines.append(f"- **Agent / Model**: `{report.agent_info.get('name', 'LibrAgent')} {report.agent_info.get('version', '')}` (`{report.agent_info.get('model_name', 'unknown')}`)")
    lines.append(f"- **Total Steps**: **{report.total_steps}** (User: {report.user_turns_count}, Agent: {report.agent_turns_count})")
    lines.append(f"- **Token Usage**: Prompt: **{report.prompt_tokens:,}** | Completion: **{report.completion_tokens:,}** | Cached: **{report.cached_tokens:,}** ({report.cache_hit_ratio:.1f}% hit)")
    total_calls = sum(report.tool_counts.values())
    total_errors = sum(report.tool_errors.values())
    lines.append(f"- **Total Tool Calls**: **{total_calls}** across **{len(report.tool_counts)}** distinct tools")
    lines.append(f"- **Tool Failure Rate**: **{total_errors} / {total_calls}** ({total_errors / max(1, total_calls) * 100:.1f}%)")
    lines.append(f"- **Session Polling (`agent__checkSession`)**: **{report.polling_check_count}** calls")
    lines.append("")

    lines.append("## 2. Key Bottleneck Findings")
    lines.append("")
    if report.findings:
        for f in report.findings:
            lines.append(f"### `[{f['level']}]` {f['title']}")
            lines.append(f"- **Category**: `{f['type']}`")
            lines.append(f"- **Evidence & Impact**: {f['detail']}")
            lines.append("")
    else:
        lines.append("No critical system-level bottlenecks flagged.")
        lines.append("")

    lines.append("## 3. Tool Execution & Observation Telemetry")
    lines.append("")
    lines.append("| Tool Name | Invocations | Errors | Failure Rate | Output Volume (KB) |")
    lines.append("|---|---|---|---|---|")
    for fn, count in report.tool_counts.most_common():
        errs = report.tool_errors.get(fn, 0)
        rate = (errs / count * 100.0) if count > 0 else 0.0
        kb = report.tool_bytes.get(fn, 0) / 1024.0
        lines.append(f"| `{fn}` | {count} | {errs} | {rate:.1f}% | {kb:.1f} KB |")
    lines.append("")

    if report.error_categories:
        lines.append("### Top Failure Categories")
        lines.append("")
        lines.append("| Category / Symptom | Occurrences |")
        lines.append("|---|---|")
        for cat, cnt in report.error_categories.most_common(top_n):
            lines.append(f"| `{cat}` | {cnt} |")
        lines.append("")

    if report.subsessions:
        lines.append("### Subsession & Lineage Activity")
        lines.append("")
        lines.append("| Subsession ID | Messages Sent | Polling Checks | Errors |")
        lines.append("|---|---|---|---|")
        for sid, sub in sorted(report.subsessions.items(), key=lambda x: x[1].checks_polled, reverse=True):
            lines.append(f"| `{sid}` | {sub.messages_sent} | {sub.checks_polled} | {sub.errors} |")
        lines.append("")

    if report.repeated_call_loops:
        lines.append("### Detected Polling & Stuck Loops (≥3 Consecutive Calls)")
        lines.append("")
        lines.append("| Tool Name | Repeat Count | Step Range |")
        lines.append("|---|---|---|")
        for loop in report.repeated_call_loops[:top_n]:
            lines.append(f"| `{loop['function_name']}` | {loop['count']} | Step {loop['start_step']} ~ {loop['end_step']} |")
        lines.append("")

    lines.append("## 4. User Feedback & Friction Points")
    lines.append("")
    lines.append(f"Found **{len(report.user_turns)}** user turns. Significant friction turns where corrections or complaints occurred:")
    lines.append("")
    friction_count = 0
    for u in report.user_turns:
        if u.friction_signals:
            friction_count += 1
            preview = u.message.replace("\n", " ")[:120]
            lines.append(f"- **Step {u.step_id:3d}** (Keywords: `{', '.join(u.friction_signals)}`): \"{preview}...\"")
    if friction_count == 0:
        lines.append("*(No explicit friction keywords detected in user turns)*")
    lines.append("")

    lines.append("## 5. Prioritized Action Items")
    lines.append("")
    lines.append("### 🛠️ Tool Layer Improvements")
    lines.append("1. **Entity Idempotency (`agent__createAgent`, `planning__addTodo`)**: Implement upsert or get-or-create semantics so re-running creation does not crash the workflow with 409 Conflict.")
    lines.append("2. **Dedicated Search MCP Integration**: Expose fast search tools (`exa__search` / native web search) directly to avoid agents resorting to complex browser scripts or fragile shell scrapers.")
    lines.append("3. **Cross-Platform Preflight for Shell Calls**: Wrap CLI/PowerShell executions with runtime path resolution (`uv run`, `node`, `python`) and provide dedicated builtin tools for common external integrations (e.g. Telegram message dispatch).")
    lines.append("4. **Safe Line Replacement (`workspace__strReplace`)**: Return clear line context or diff suggestions when a replace target is duplicated or missing.")
    lines.append("")
    lines.append("### ⚙️ Harness & Orchestration Improvements")
    lines.append("1. **Event-Driven Subagent Wakeup (Anti-Polling)**: Eliminate rapid `agent__checkSession` polling loops by using long-polling timeouts, push events, or exponential backoff with sleep.")
    lines.append("2. **Strict Invariant Re-anchoring**: Enforce core user constraints (e.g. strict 24h publication time window, mandatory verified source URLs) via persistent system injection so they are not forgotten across 50+ turns.")
    lines.append("3. **Context Compaction & Pruning**: Truncate oversized raw tool observations (e.g. massive command outputs or whole-file dumps) to protect prompt token limits and budget.")
    lines.append("4. **SSOT ID Guard**: Strictly ban session ID truncation (e.g. using 10-char suffixes) across tool arguments and prompts to ensure 100% ID matching.")
    lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Trace Evaluation Loop Analyzer")
    parser.add_argument("trace_path", help="Path to trajectory JSON or .trace.json file")
    parser.add_argument("--output", "-o", help="Path to write report (markdown or json)")
    parser.add_argument("--format", choices=["markdown", "json", "summary"], default="markdown", help="Output format")
    parser.add_argument("--top", type=int, default=15, help="Number of top errors/loops to show")
    args = parser.parse_args()

    trace_file = Path(args.trace_path)
    if not trace_file.exists():
        print(f"Error: Trace file not found: {trace_file}", file=sys.stderr)
        sys.exit(1)

    report = parse_trace_file(trace_file)

    if args.format == "json":
        output_data = {
            "session_id": report.session_id,
            "agent_info": report.agent_info,
            "total_steps": report.total_steps,
            "prompt_tokens": report.prompt_tokens,
            "completion_tokens": report.completion_tokens,
            "cached_tokens": report.cached_tokens,
            "cache_hit_ratio": report.cache_hit_ratio,
            "tool_counts": dict(report.tool_counts),
            "tool_errors": dict(report.tool_errors),
            "tool_bytes": dict(report.tool_bytes),
            "error_categories": dict(report.error_categories),
            "subsessions": {k: vars(v) for k, v in report.subsessions.items()},
            "repeated_loops": report.repeated_call_loops,
            "polling_check_count": report.polling_check_count,
            "findings": report.findings,
        }
        rendered = json.dumps(output_data, indent=2, ensure_ascii=False)
    elif args.format == "summary":
        rendered = (
            f"Trace: {trace_file.name} | Steps: {report.total_steps} | "
            f"Tokens: {report.prompt_tokens:,} | Tools: {sum(report.tool_counts.values())} | "
            f"Errors: {sum(report.tool_errors.values())} | Polling checks: {report.polling_check_count}"
        )
    else:
        rendered = generate_markdown_report(report, trace_file.name, top_n=args.top)

    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(rendered)
        print(f"Report successfully saved to {out_path}")
    else:
        print(rendered)


if __name__ == "__main__":
    main()
