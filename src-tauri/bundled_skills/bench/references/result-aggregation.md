# Result Aggregation Patterns

This document defines patterns and strategies for aggregating, scoring, and analyzing benchmark results from child sessions in the `bench` skill.

---

## 1. Aggregation Strategies

Depending on the nature of the benchmark, choose the appropriate scoring strategy.

### Strategy A: Test-Execution Verification (Automated)

The most rigorous validation method. The child session runs a verification suite (e.g., unit tests) locally, and the parent parses the terminal/test outputs.

*   **When to use:** Coding benchmarks (e.g., SWE-bench, LeetCode challenges) where tests are available.
*   **Verification Flow:**
    1.  Worker agent modifies the codebase to fix the bug.
    2.  Worker agent executes `test_command` (e.g., `pytest`, `npm test`).
    3.  Parent inspects the output of the final turn.
*   **Parsing Exit Codes / Output:**
    *   **Pass:** Exit code `0` and test suite reports `PASSED` / `OK`.
    *   **Fail:** Exit code `> 0` or assertions fail (e.g., `AssertionError`).
    *   **Score:** `1.0` for all tests passing, `0.0` for failures.

### Strategy B: LLM Judge-Based Evaluation (Semantic)

Use a dedicated critic or judge session (e.g., a "Master Mind" agent) to compare the child's final answer against the expected reference.

*   **When to use:** Open-ended tasks, creative writing, planning, or complex code refactorings without unit tests.
*   **Judge Prompt Template:**
    ```markdown
    You are an objective AI evaluator. Rate the agent's submission on a scale of 0.0 to 1.0.
    
    [Problem Task]
    ${problem.task}
    
    [Expected Reference Output]
    ${problem.expected_output}
    
    [Agent Submission]
    ${workerAnswer}
    
    Evaluate the submission against the expected output. Provide:
    1. A numeric score (0.0 to 1.0)
    2. A brief analysis explaining the score
    3. A label: "passed" (score >= 0.8), "failed" (score < 0.8)
    
    Return your response strictly in the following JSON format:
    {
      "score": <number>,
      "label": "passed" | "failed",
      "analysis": "<string>"
    }
    ```

### Strategy C: Exact / Regex Match (Syntactic)

A simple, fast comparison of the agent's final textual response with `expected_output` using text comparisons.

*   **When to use:** Math problems, multiple-choice questions, or short-answer tests.
*   **Matching Rules:**
    *   **Exact Match:** String equality after trimming whitespace and lowercase normalization.
    *   **Regex Match:** Match patterns (e.g., `/\b42\b/`).
    *   **Score:** `1.0` if matched, `0.0` otherwise.

---

## 2. Handling Failures and Timeouts

Ensure failures and timeouts are cleanly categorized and aggregated in the final report.

| Condition | Aggregated Status | Score | Cause | Action Needed |
|---|---|---|---|---|
| Child runs out of time | `timeout` | `0.0` | Session duration exceeded `timeout` limit | Call `agent__stopSession(sessionId)` to reclaim CPU/Memory |
| Child crashes with error | `error` | `0.0` | API errors, runtime crashes, out of memory | Log error message to report |
| Child completes but fails tests | `failed` | `0.0` or `< 1.0` | Output does not match expectation or tests failed | Parse failed test case names for error logs |

---

## 3. Consolidated Data Schema

The parent session should structure all parsed results into a standardized JSON payload before generating the markdown report.

```json
{
  "benchmark": "swe-bench-lite",
  "startTime": "2026-06-13T00:00:00Z",
  "endTime": "2026-06-13T00:15:30Z",
  "summary": {
    "total": 30,
    "passed": 22,
    "failed": 6,
    "errors": 2,
    "passRate": 0.733,
    "averageLatencyMs": 45000,
    "totalTokensUsed": 452000
  },
  "results": [
    {
      "problemId": "django__django-11890",
      "sessionId": "session_a8f902c3",
      "status": "passed",
      "score": 1.0,
      "latencyMs": 45000,
      "tokensUsed": 15000,
      "error": null,
      "analysis": "Agent successfully modified django/forms/fields.py and all 3 unit tests passed."
    },
    {
      "problemId": "flask-2048",
      "sessionId": "session_b7c2d109",
      "status": "failed",
      "score": 0.0,
      "latencyMs": 120000,
      "tokensUsed": 32000,
      "error": "AssertionError: 1 != 2",
      "analysis": "The agent introduced a syntax error when handling JSON serialization of NaN values."
    }
  ]
}
```
