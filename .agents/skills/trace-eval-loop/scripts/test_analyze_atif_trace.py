#!/usr/bin/env python3
"""Unit tests for analyze_atif_trace.py."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from analyze_atif_trace import (
    aggregate,
    analyze_trajectory,
    discover_paths,
    load_json,
    main,
    render_markdown,
)

FIXTURE = Path(__file__).parent / "fixtures" / "sample_trajectory.json"


class AnalyzeAtifTraceTests(unittest.TestCase):
    def test_analyze_fixture(self) -> None:
        payload, error = load_json(FIXTURE)
        self.assertIsNone(error)
        assert payload is not None
        summary = analyze_trajectory(
            FIXTURE, payload, oversize_chars=8000, sequence_limit=80
        )
        self.assertEqual(summary["schema_version"], "ATIF-v1.7")
        self.assertEqual(summary["tool_calls"], 3)
        self.assertEqual(summary["repeated_adjacent_calls"], 1)
        self.assertGreaterEqual(summary["heuristic_error_observations"], 2)
        self.assertIn("workspace__searchFiles", summary["per_tool"])
        self.assertIn("workspace__notARealTool", summary["unknown_tools_heuristic"])
        self.assertTrue(
            any(event["kind"] == "tool_call" for event in summary["sequence"])
        )

    def test_discover_directory(self) -> None:
        paths = discover_paths([str(FIXTURE.parent)])
        self.assertTrue(any(p.name == "sample_trajectory.json" for p in paths))

    def test_aggregate_and_markdown(self) -> None:
        payload, _ = load_json(FIXTURE)
        assert payload is not None
        summary = analyze_trajectory(
            FIXTURE, payload, oversize_chars=8000, sequence_limit=80
        )
        report = aggregate([summary], [])
        md = render_markdown(report)
        self.assertIn("Session ATIF trace summary", md)
        self.assertIn("workspace__searchFiles", md)

    def test_main_writes_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_json = Path(tmp) / "summary.json"
            out_md = Path(tmp) / "summary.md"
            code = main(
                [
                    str(FIXTURE),
                    "--output",
                    str(out_json),
                    "--markdown",
                    str(out_md),
                ]
            )
            self.assertEqual(code, 0)
            data = json.loads(out_json.read_text(encoding="utf-8"))
            self.assertEqual(data["trajectory_count"], 1)
            self.assertTrue(out_md.read_text(encoding="utf-8").startswith("#"))


if __name__ == "__main__":
    unittest.main()
