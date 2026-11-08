"""O-10: nested writeFile under a file path must fail (independent of assistant prose)."""

from pathlib import Path

from verifier_lib import load_trajectory, observation_result_texts, trajectory_steps

NESTED = Path("/app/parent.txt/child.txt")


def test_parent_file_exists():
    assert Path("/app/parent.txt").is_file()


def test_nested_child_not_created():
    assert not NESTED.exists()


def test_writefile_error_in_tool_results():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("existing file" in t.lower() for t in texts), texts[:3]
