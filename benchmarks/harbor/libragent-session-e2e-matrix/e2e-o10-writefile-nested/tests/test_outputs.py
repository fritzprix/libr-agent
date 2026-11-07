"""O-10: nested writeFile under a file path must fail (independent of assistant prose)."""

from pathlib import Path

from verifier_lib import load_trajectory, trajectory_steps

NESTED = Path("/app/parent.txt/child.txt")


def test_parent_file_exists():
    assert Path("/app/parent.txt").is_file()


def test_nested_child_not_created():
    assert not NESTED.exists()


def test_writefile_error_in_tool_results():
    steps = trajectory_steps(load_trajectory())
    texts = []
    for step in steps:
        for call in step.get("tool_calls") or []:
            result = call.get("result") or {}
            for item in result.get("content") or []:
                if isinstance(item, dict) and item.get("text"):
                    texts.append(item["text"])
    assert any("existing file" in t.lower() for t in texts), texts[:3]
