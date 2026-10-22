"""Tests for check-layman-intent-trajectories.py.

Run: python3 -m unittest scripts/test_check_layman_intent_trajectories.py
"""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "layman_checker", Path(__file__).with_name("check-layman-intent-trajectories.py")
)
assert _SPEC is not None and _SPEC.loader is not None
checker = importlib.util.module_from_spec(_SPEC)
# dataclasses resolve annotations through sys.modules.
sys.modules[_SPEC.name] = checker
_SPEC.loader.exec_module(checker)


def _call(name: str, arguments: object) -> dict[str, object]:
    return {"tool_call_id": name, "function_name": name, "arguments": arguments}


def _write_trial(job: Path, trial: str, steps: list[dict[str, object]]) -> Path:
    path = job / trial / "agent" / "trajectory.json"
    path.parent.mkdir(parents=True)
    path.write_text(
        json.dumps({"schema_version": "1", "steps": steps}), encoding="utf-8"
    )
    return path


class GradeTests(unittest.TestCase):
    def test_t01_passes_with_loop_skill_and_ten_minute_callback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = _write_trial(
                Path(tmp),
                "t01-water__abc",
                [
                    {
                        "source": "agent",
                        "message": "",
                        "tool_calls": [
                            _call(
                                "workspace__readFile",
                                {"path": "@system-skills/loop/SKILL.md"},
                            ),
                            _call(
                                "scheduled_task__scheduleCallback",
                                '{"delaySeconds": 600, "message": "drink"}',
                            ),
                        ],
                    }
                ],
            )
            result = checker.grade(
                checker.P0_EXPECTATIONS["T01"], checker.load_trace(path)
            )
        self.assertTrue(result.passed, result.failures)

    def test_t01_fails_on_wrong_scheduler_and_delay(self) -> None:
        trace = checker.Trace(
            tool_calls=[
                checker.ToolCall(
                    "workspace__readFile", {"path": "@system-skills/schedule/SKILL.md"}
                ),
                checker.ToolCall(
                    "scheduled_task__scheduleCallback", {"delaySeconds": 3600}
                ),
                checker.ToolCall("scheduled_task__createScheduledTask", {}),
            ]
        )
        failures = checker.grade(checker.P0_EXPECTATIONS["T01"], trace).failures
        self.assertTrue(any("skill loop not read" in f for f in failures), failures)
        self.assertIn("forbidden call __createScheduledTask", failures)
        self.assertTrue(any("delaySeconds≈600" in f for f in failures), failures)

    def test_q_task_requires_question_and_no_side_effects(self) -> None:
        asked = checker.Trace(final_agent_message="시간 기준인가요, 이벤트 기준인가요?")
        self.assertTrue(checker.grade(checker.P0_EXPECTATIONS["T05"], asked).passed)

        silent = checker.Trace(
            tool_calls=[checker.ToolCall("scheduled_task__scheduleCallback", {})],
            final_agent_message="알림을 설정했습니다.",
        )
        failures = checker.grade(checker.P0_EXPECTATIONS["T05"], silent).failures
        self.assertIn("forbidden call __scheduleCallback", failures)
        self.assertIn("Q task: final reply does not ask a question", failures)

    def test_renamed_skill_alias_counts_as_expected_skill(self) -> None:
        trace = checker.Trace(
            tool_calls=[
                checker.ToolCall(
                    "workspace__readFile",
                    {"path": "@system-skills/tool-installer/SKILL.md"},
                ),
                checker.ToolCall("tool__registerServer", {}),
            ]
        )
        self.assertTrue(checker.grade(checker.P0_EXPECTATIONS["T06"], trace).passed)

    def test_n_task_fails_when_sub_session_spawned(self) -> None:
        trace = checker.Trace(
            tool_calls=[checker.ToolCall("session__spawnSession", {})]
        )
        self.assertFalse(checker.grade(checker.P0_EXPECTATIONS["T15"], trace).passed)


class DiscoveryTests(unittest.TestCase):
    def test_main_reports_per_skill_and_exit_code(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            jobs = Path(tmp)
            job = jobs / "2026-10-22__layman"
            _write_trial(
                job,
                "t15-typo__1",
                [{"source": "agent", "message": "Fixed.", "tool_calls": []}],
            )
            _write_trial(
                job,
                "t05-later__1",
                [{"source": "agent", "message": "Done.", "tool_calls": []}],
            )
            self.assertEqual(checker.main(["--jobs-dir", str(jobs)]), 1)

            trials = {task_id for task_id, _ in checker.find_trials(job)}
            self.assertEqual(trials, {"T05", "T15"})

    def test_task_id_requires_two_digit_token(self) -> None:
        self.assertEqual(checker.task_id_for(Path("t03-weekday-summary__x")), "T03")
        self.assertIsNone(checker.task_id_for(Path("terminal-bench-123")))


if __name__ == "__main__":
    unittest.main()
