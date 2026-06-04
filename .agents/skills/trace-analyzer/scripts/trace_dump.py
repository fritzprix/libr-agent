#!/usr/bin/env python3
"""
trace_dump.py — LibrAgent session trace analyzer
Usage: python trace_dump.py <path-to-.trace.json>
"""
import json
import sys
from collections import Counter


def load(path: str):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def first_line(text: str, n: int = 150) -> str:
    return text.split("\n")[0][:n]


def short_id(s: str, n: int = 10) -> str:
    return (s or "")[-n:]


def main():
    # Force UTF-8 output to handle unicode tool result markers on Windows (CP949)
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]

    path = sys.argv[1] if len(sys.argv) > 1 else ".trace.json"
    trace = load(path)

    roles = Counter(m["role"] for m in trace)
    print(f"=== OVERVIEW ===")
    print(f"Total messages : {len(trace)}")
    print(f"Roles          : {dict(roles)}")

    # ── Tool call frequency ─────────────────────────────────────────────────
    tool_names = []
    for m in trace:
        if m["role"] == "assistant" and m.get("tool_calls"):
            for tc in m["tool_calls"]:
                if not isinstance(tc, dict):
                    continue
                name = (
                    tc.get("function", {}).get("name")
                    or tc.get("name")
                    or "?"
                )
                tool_names.append(name)

    freq = Counter(tool_names)
    print(f"\n=== TOOL CALL FREQUENCY ({sum(freq.values())} total) ===")
    for name, count in freq.most_common():
        print(f"  {count:>4}x  {name}")

    # ── Errors ──────────────────────────────────────────────────────────────
    errors = []
    for m in trace:
        if m["role"] == "tool" and m.get("content"):
            tcid = short_id(m.get("tool_call_id") or "")
            for c in m["content"]:
                if isinstance(c, dict) and c.get("type") == "text":
                    txt = c["text"]
                    if txt.startswith("✗") or "Error:" in txt or "not found" in txt.lower():
                        errors.append(f"  [{tcid}] {first_line(txt)}")
                    break

    print(f"\n=== ERRORS ({len(errors)}) ===")
    for e in errors:
        print(e)

    # ── All tool calls with args ─────────────────────────────────────────────
    print(f"\n=== TOOL CALLS ===")
    for m in trace:
        if m["role"] == "assistant" and m.get("tool_calls"):
            for tc in m["tool_calls"]:
                if not isinstance(tc, dict):
                    continue
                name = (
                    tc.get("function", {}).get("name")
                    or tc.get("name")
                    or "?"
                )
                raw = tc.get("function", {}).get("arguments") or tc.get("arguments") or ""
                try:
                    args = json.dumps(
                        json.loads(raw) if isinstance(raw, str) else raw,
                        ensure_ascii=False,
                    )[:120]
                except Exception:
                    args = str(raw)[:120]
                print(f"  {name}({args})")

    # ── Tool results ─────────────────────────────────────────────────────────
    print(f"\n=== TOOL RESULTS ===")
    for m in trace:
        if m["role"] == "tool" and m.get("content"):
            tcid = short_id(m.get("tool_call_id") or "")
            for c in m["content"]:
                if isinstance(c, dict) and c.get("type") == "text":
                    print(f"  [{tcid}] {first_line(c['text'])}")
                    break

    # ── Assistant text turns ─────────────────────────────────────────────────
    print(f"\n=== ASSISTANT TEXT ===")
    for m in trace:
        if m["role"] == "assistant" and m.get("content"):
            for c in m["content"]:
                if isinstance(c, dict) and c.get("type") == "text" and c["text"].strip():
                    print(c["text"][:400])
                    print("---")
                    break


if __name__ == "__main__":
    main()
