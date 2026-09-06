#!/usr/bin/env python3
"""Unit tests for analyze_jobs_trace.py."""

import json
import tempfile
import unittest
from pathlib import Path

from analyze_jobs_trace import (
    categorize_failure,
    extract_tools_from_atif,
    extract_tools_from_metadata_trajectory,
    resolve_reward,
)


class TestAnalyzeJobsTrace(unittest.TestCase):
    def test_categorize_success(self):
        cat = categorize_failure(
            reward=1.0,
            exception_name=None,
            turns_count=10,
            explicit_tool_errors=[],
            repeated_adjacent_calls=0,
        )
        self.assertEqual(cat, "SUCCESS")

    def test_categorize_timeout(self):
        cat = categorize_failure(
            reward=0.0,
            exception_name="TimeoutError: command timed out",
            turns_count=10,
            explicit_tool_errors=[],
            repeated_adjacent_calls=0,
        )
        self.assertEqual(cat, "TIMEOUT_EXCEEDED")

    def test_categorize_network_api_error(self):
        cat = categorize_failure(
            reward=0.0,
            exception_name="openai.APIConnectionError: Connection error",
            turns_count=5,
            explicit_tool_errors=[],
            repeated_adjacent_calls=0,
        )
        self.assertEqual(cat, "NETWORK_API_ERROR")

    def test_network_api_error_does_not_mismatch_unrelated_api(self):
        cat = categorize_failure(
            reward=0.0,
            exception_name="MyCustomFastAPIUserError",
            turns_count=15,
            explicit_tool_errors=[],
            repeated_adjacent_calls=0,
        )
        # Should NOT be classified as NETWORK_API_ERROR
        self.assertEqual(cat, "VERIFIER_FAILED_WRONG_STATE")

    def test_categorize_early_abort_zero_turns(self):
        cat = categorize_failure(
            reward=0.0,
            exception_name=None,
            turns_count=0,
            explicit_tool_errors=[],
            repeated_adjacent_calls=0,
        )
        self.assertEqual(cat, "INITIALIZATION_OR_EARLY_ABORT")

    def test_categorize_early_tight_loop(self):
        # Even before turn 30, repeated adjacent calls >= 3 should be flagged as stuck
        cat = categorize_failure(
            reward=0.0,
            exception_name=None,
            turns_count=12,
            explicit_tool_errors=[],
            repeated_adjacent_calls=3,
        )
        self.assertEqual(cat, "AGENT_LOOP_OR_STUCK")

    def test_categorize_high_turn_count_without_loop(self):
        cat = categorize_failure(
            reward=0.0,
            exception_name=None,
            turns_count=35,
            explicit_tool_errors=[],
            repeated_adjacent_calls=1,
        )
        self.assertEqual(cat, "HIGH_TURN_COUNT")

    def test_extract_tools_from_metadata_trajectory_with_repeats(self):
        trajectory = [
            {
                "role": "assistant",
                "toolCalls": [
                    {"function": {"name": "read_file", "arguments": {"path": "a.txt"}}}
                ],
            },
            {
                "role": "assistant",
                "toolCalls": [
                    {"function": {"name": "read_file", "arguments": {"path": "a.txt"}}}
                ],
            },
            {
                "role": "assistant",
                "toolCalls": [
                    {"function": {"name": "read_file", "arguments": {"path": "a.txt"}}}
                ],
            },
            {
                "role": "tool",
                "error": "File not found",
            },
        ]
        tools, errors, repeats = extract_tools_from_metadata_trajectory(trajectory)
        self.assertEqual(tools, ["read_file", "read_file", "read_file"])
        self.assertEqual(errors, ["File not found"])
        self.assertEqual(repeats, 2)

    def test_extract_tools_from_atif(self):
        atif = {
            "steps": [
                {
                    "tool_calls": [
                        {"function_name": "bash", "arguments": {"cmd": "ls"}}
                    ],
                    "observation": {"results": [{"content": "file1 file2"}]},
                },
                {
                    "tool_calls": [
                        {"function_name": "bash", "arguments": {"cmd": "ls"}}
                    ],
                    "observation": {"results": [{"content": "error: failed to read"}]},
                },
            ]
        }
        tools, steps, heuristics, repeats = extract_tools_from_atif(atif)
        self.assertEqual(tools, ["bash", "bash"])
        self.assertEqual(steps, 2)
        self.assertEqual(heuristics, 1)  # keyword 'error: failed to read'
        self.assertEqual(repeats, 1)

    def test_resolve_reward_priority(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            td = Path(tmpdir)
            verifier_dir = td / "verifier"
            verifier_dir.mkdir()
            (verifier_dir / "reward.txt").write_text("1.0", encoding="utf-8")

            # Even if result_json has 0.0, reward.txt must take precedence as SSOT
            result_json = {"verifier_result": {"rewards": {"reward": 0.0}}}
            reward = resolve_reward(td, result_json)
            self.assertEqual(reward, 1.0)


if __name__ == "__main__":
    unittest.main()
