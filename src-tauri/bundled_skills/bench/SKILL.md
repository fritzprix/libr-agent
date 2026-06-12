---
name: bench
description: |
  Run benchmark-style evaluations on AI agents or tools using parent-child session delegation.
  Use when the user wants to test an agent's performance on a set of problems (e.g. SWE-bench style,
  tool capability benchmark, coding challenge evaluation). The parent session orchestrates:
  spawns child sessions to solve individual problems, collects answers via checkSession,
  and generates a consolidated report with pass/fail scores and analysis.
  Triggers on: "bench 테스트 해줘", "SWE-bench 돌려줘", "agent benchmark", "tool evaluation".
---

# Bench

Run **benchmark evaluations** by delegating individual problems to child sessions and aggregating results into a report.

This skill turns LibrAgent's parent-child delegation primitives into a structured benchmarking workflow.

## Not This Skill

| Skill | Use for |
| --- | --- |
| **mcp-builder** | Build a new MCP server (not testing one) |
| **system-setup** | Install runtime dependencies |
| **deep-research** | Multi-source research with web search |
| **consensus-delegation** | Multi-expert opinion triangulation on a single question |

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│              Parent Session                 │
│           (Orchestrator / Bench Agent)      │
│                                             │
│  1. Load benchmark definition               │
│  2. Spawn child sessions (one per problem)  │
│  3. Monitor progress with checkSession      │
│  4. Collect results                         │
│  5. Generate consolidated report            │
└──────┬──────────┬──────────┬────────────────┘
       │          │          │
  ┌────▼───┐ ┌───▼────┐ ┌──▼──────┐
  │Child 1 │ │Child 2 │ │Child N  │
  │(Worker)│ │(Worker)│ │(Worker) │
  └────────┘ └────────┘ └─────────┘
```

**Key primitives used:**

| Primitive | Role |
|---|---|
| `agent__list(type="configs")` | Find the assistant to bench |
| `agent__startSession(task="...")` | Spawn a child worker for one problem |
| `agent__checkSession(sessionId, wait=true)` | Block until a child finishes, get its answer |
| `agent__checkSession(sessionId)` (poll) | Monitor progress without blocking |
| `agent__stopSession(sessionId)` | Cancel a stuck child |
| `agent__list(type="sessions")` | See all active children |

## Benchmark Definition Format

Define problems as a JSON array or Markdown list. Each problem has:

```json
{
  "name": "swe-bench-lite",
  "description": "SWE-bench Lite verification set",
  "assistant": "Coding Expert",
  "problems": [
    {
      "id": "django__django-11890",
      "task": "Fix the validation error in django/forms/fields.py when...",
      "repository": "/path/to/django",
      "expected_output": "The field should accept null values",
      "difficulty": "medium"
    }
  ]
}
```

Or use a simple text format:

```
Benchmark: Python Math Test
Assistant: Coding Expert
Problems:
  1. [easy] Solve: 2 + 2 * 3 = ?
  2. [medium] Write a function to calculate Fibonacci(n)
  3. [hard] Implement a binary search on a rotated sorted array
```

## Workflow

### 1. Load benchmark definition

Parse the benchmark from user input, a file, or a URL.

```
// If user provides a file path
workspace__readFile(path="benchmarks/swe-bench.json")

// Or parse from chat message
// Extract name, assistant, problems array
```

Record: `benchmark.name`, `benchmark.assistant`, `benchmark.problems[]`.

### 2. Choose the assistant to bench

```
agent__list(type="configs", query="Coding Expert")
```

Record the `id` for `agent__startSession`. If the user names a custom assistant, use that ID.

### 3. Spawn child sessions

For each problem, spawn a child session:

```
agent__startSession({
  agentId: "<assistant-id>",
  task: `<benchmark-task>\n\nProblem ID: ${problem.id}\nTask: ${problem.task}\nRepository: ${problem.repository}\nExpected output: ${problem.expected_output}`,
  waitForResult: false,
  includeCurrentOrg: false   // Children don't need to appear in org view
})
```

**Important:** Each child gets its own isolated workspace by default. If the problem requires a shared repository, use `workspaceOverride`.

Record all returned `sessionId` values in a map: `sessions[problem.id] = sessionId`.

### 4. Monitor progress

Poll children without blocking:

```
// Check all children every 30 seconds
for (const [id, sessionId] of Object.entries(sessions)) {
  const status = await agent__checkSession(sessionId);
  if (status.state === "completed") {
    results[id] = status.lastResponse;
  } else if (status.state === "error") {
    results[id] = `Error: ${status.error}`;
  }
}
```

Or wait for a specific child:

```
const result = await agent__checkSession(sessionId, wait=true, timeout=300);
```

### 5. Collect results

For each problem, the result is the child's final response. Parse it:

```typescript
interface BenchmarkResult {
  problemId: string;
  sessionId: string;
  status: "passed" | "failed" | "error" | "timeout";
  answer: string;
  score: number;       // 0-1 or percentage
  latencyMs: number;   // time to complete
  tokensUsed?: number;
}
```

### 6. Generate report

Produce a Markdown report:

```markdown
# Benchmark Report: {name}

## Summary
- **Total:** N problems
- **Passed:** X ({X/N}%)
- **Failed:** Y
- **Errors:** Z
- **Avg Latency:** T ms

## Results

| # | Problem | Status | Score | Latency |
|---|---------|--------|-------|---------|
| 1 | django-11890 | ✅ passed | 1.0 | 45s |
| 2 | flask-2048 | ❌ failed | 0.0 | 120s |
| 3 | ... | ⚠️ error | - | timeout |

## Failed Problems

### django-11890
**Expected:** The field should accept null values
**Got:** {child's answer snippet}
**Analysis:** The agent failed to handle the edge case because...

## Errors

### flask-2048
**Error:** {error message}
**Session:** {sessionId}
```

## Advanced Patterns

### Parallel vs Sequential

| Mode | When | How |
|---|---|---|
| **Parallel** (default) | Independent problems | Spawn all children, poll with `checkSession` |
| **Sequential** | Dependencies between problems | Wait for each child before spawning next |

### Timeout Handling

```
// If a child exceeds timeout
const status = await agent__checkSession(sessionId, wait=true, timeout=300);
if (status.state === "running" && status.timedOut) {
  await agent__stopSession(sessionId);
  results[id] = { status: "timeout", answer: "Timed out after 300s" };
}
```

### Fanout Limit

By default, child sessions inherit the parent's `maxFanout`. If you need to bench many problems:

```
agent__startSession({
  agentId: "...",
  task: "...",
  maxFanout: 20   // Override parent limit for this child
})
```

### Result Verification

If the benchmark has an expected answer (e.g. unit test output), compare:

```
// Use a separate child session as "judge"
agent__startSession({
  agentId: "Master Mind",  // or a dedicated judge agent
  task: `Evaluate this answer against the expected output.\n\nProblem: ${problem.task}\nExpected: ${problem.expected_output}\nGot: ${workerAnswer}\n\nReturn a score 0-1 and brief analysis.`
})
```

## Script Reference

| Step | Tool | Parameters |
|---|---|---|
| Find assistant | `agent__list` | `{ type: "configs", query: "..." }` |
| Spawn child | `agent__startSession` | `{ agentId, task, waitForResult: false }` |
| Poll child | `agent__checkSession` | `{ sessionId }` |
| Wait for child | `agent__checkSession` | `{ sessionId, wait: true, timeout: 300 }` |
| Stop stuck child | `agent__stopSession` | `{ sessionId }` |
| List children | `agent__list` | `{ type: "sessions" }` |
| Read file | `workspace__readFile` | `{ path: "benchmarks/..." }` |

## Guidelines

- **Isolate workspaces** — children should not share the parent's workspace unless explicitly needed (`workspaceOverride`).
- **Set timeouts** — always specify `timeout` on `checkSession(wait=true)` to prevent infinite waits.
- **Limit fanout** — be mindful of `maxFanout` limits; spawn in batches if needed.
- **Verify results** — don't trust child answers blindly; use a judge session for quality checks.
- **Clean up** — stop any remaining children after the benchmark completes.
- **Report format** — use consistent Markdown tables for readability.
- **English output** — benchmark reports in English unless the user specifies otherwise.

## References

- [benchmark-templates.md](references/benchmark-templates.md) — example benchmark definitions
- [result-aggregation.md](references/result-aggregation.md) — patterns for scoring and analysis
