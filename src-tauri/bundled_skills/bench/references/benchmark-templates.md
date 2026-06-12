# Benchmark Templates

Example benchmark definitions for common evaluation scenarios.

## SWE-bench Lite Style

```json
{
  "name": "swe-bench-lite",
  "description": "SWE-bench Lite verification set — 300 resolved issues",
  "assistant": "Coding Expert",
  "problems": [
    {
      "id": "django__django-11890",
      "task": "Fix the validation error in django/forms/fields.py when the field value is None.",
      "repository": "/path/to/django",
      "setup": "cd /path/to/django && pip install -e .[test]",
      "test_command": "python -m pytest tests/forms/tests.py::TestFields::test_null_field",
      "expected_output": "PASSED",
      "difficulty": "medium"
    },
    {
      "id": "flask-2048",
      "task": "Fix the JSON serialization bug when response contains NaN values.",
      "repository": "/path/to/flask",
      "setup": "cd /path/to/flask && pip install -e .[test]",
      "test_command": "python -m pytest tests/test_json.py",
      "expected_output": "PASSED",
      "difficulty": "easy"
    }
  ]
}
```

## Tool Capability Benchmark

```json
{
  "name": "tool-capability-test",
  "description": "Test all builtin tool capabilities of a given assistant",
  "assistant": "Libr Assistant",
  "problems": [
    {
      "id": "tool-planning",
      "task": "Use the planning tools to create a goal 'Build a web app', add 3 todos, mark first as done, then reflect on progress.",
      "expected_output": "Goal created, 3 todos added, 1 done, reflection recorded",
      "difficulty": "easy"
    },
    {
      "id": "tool-workspace-read",
      "task": "Read the file 'README.md' from the workspace root and return the first 10 lines.",
      "expected_output": "First 10 lines of README.md",
      "difficulty": "easy"
    },
    {
      "id": "tool-workspace-edit",
      "task": "Create a file 'bench-output.txt' with content 'Benchmark test passed' and verify it exists.",
      "expected_output": "File created and verified",
      "difficulty": "easy"
    },
    {
      "id": "tool-agent-delegate",
      "task": "List all agent configs and return the count.",
      "expected_output": "Count of agent configurations",
      "difficulty": "medium"
    },
    {
      "id": "tool-shell-command",
      "task": "Run 'echo hello bench' and return the output.",
      "expected_output": "hello bench",
      "difficulty": "easy"
    }
  ]
}
```

## Code Challenge Benchmark

```markdown
Benchmark: Python Coding Challenges
Assistant: Coding Expert
Problems:
  1. [easy] Write a function `fib(n)` that returns the nth Fibonacci number. Handle n < 0 by returning -1.
  2. [medium] Implement a binary search on a rotated sorted array. Return the index or -1 if not found.
  3. [hard] Given a list of intervals, merge overlapping intervals. Input: [[1,3],[2,6],[8,10],[15,18]] → Output: [[1,6],[8,10],[15,18]]
  4. [hard] Implement LRU Cache with O(1) get and put operations.
```

## Scoring Rubric

| Score | Meaning |
|---|---|
| `1.0` | Fully correct — output matches expected exactly |
| `0.7` | Partially correct — logic is right but edge cases missed |
| `0.3` | Mostly wrong — correct approach but critical bugs |
| `0.0` | Failed — wrong answer or error |
| `-1` | Error/timeout — child session crashed or timed out |
